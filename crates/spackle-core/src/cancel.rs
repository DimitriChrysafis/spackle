//! Cancellation token: one shared, cheaply-clonable cancel signal.
//!
//! The loop, transport, and subprocess supervision all hold clones of the
//! same token. Cancelling it must let every waiter wake and drop work. This
//! type is std-only (no tokio) so it can be used from synchronous code and
//! held across awaits.

use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

/// A cloneable, thread-safe cancellation signal.
#[derive(Debug, Clone)]
pub struct CancellationToken {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    flag: AtomicBool,
    mutex: Mutex<()>,
    condvar: Condvar,
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl CancellationToken {
    /// Create a fresh, uncancelled token.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                flag: AtomicBool::new(false),
                mutex: Mutex::new(()),
                condvar: Condvar::new(),
            }),
        }
    }

    /// Whether cancellation has been requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.inner.flag.load(Ordering::Acquire)
    }

    /// Request cancellation, waking all waiters.
    pub fn cancel(&self) {
        self.inner.flag.store(true, Ordering::Release);
        self.inner.condvar.notify_all();
    }

    /// Wait up to `timeout` for cancellation. Returns true if cancelled.
    pub fn wait_timeout(&self, timeout: Duration) -> bool {
        let mut guard = self.inner.mutex.lock().expect("cancel lock poisoned");
        let deadline = Instant::now() + timeout;
        loop {
            if self.is_cancelled() {
                return true;
            }
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            match self.inner.condvar.wait_timeout(guard, deadline - now) {
                std::result::Result::Ok((g, _)) => guard = g,
                std::result::Result::Err(_) => return self.is_cancelled(),
            }
        }
    }

    /// Block until cancelled.
    pub fn block_until_cancelled(&self) {
        let guard = self.inner.mutex.lock().expect("cancel lock poisoned");
        if let Err(err) = self
            .inner
            .condvar
            .wait_while(guard, |_| !self.is_cancelled())
        {
            let _poison = err;
        }
    }

    /// Return a child token that is already cancelled if this one is, and
    /// stays independent thereafter (used to scope one operation).
    #[must_use]
    pub fn scoped(&self) -> Self {
        if self.is_cancelled() {
            let child = Self::new();
            child.cancel();
            child
        } else {
            Self::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_uncancelled() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
    }

    #[test]
    fn cancel_is_visible_to_clones() {
        let token = CancellationToken::new();
        let clone = token.clone();
        assert!(!clone.is_cancelled());
        token.cancel();
        assert!(clone.is_cancelled());
    }

    #[test]
    fn wait_times_out_when_not_cancelled() {
        let token = CancellationToken::new();
        let started = Instant::now();
        let cancelled = token.wait_timeout(Duration::from_millis(30));
        assert!(!cancelled);
        assert!(started.elapsed() >= Duration::from_millis(20));
    }

    #[test]
    fn wait_returns_promptly_when_cancelled_elsewhere() {
        let token = CancellationToken::new();
        let waiter = token.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(10));
            waiter.cancel();
        });
        let started = Instant::now();
        let cancelled = token.wait_timeout(Duration::from_secs(2));
        assert!(cancelled);
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn scoped_inherits_when_cancelled() {
        let token = CancellationToken::new();
        token.cancel();
        let child = token.scoped();
        assert!(child.is_cancelled());
    }

    #[test]
    fn scoped_independent_when_active() {
        let token = CancellationToken::new();
        let child = token.scoped();
        assert!(!child.is_cancelled());
        child.cancel();
        assert!(!token.is_cancelled(), "child must not cancel parent");
    }
}
