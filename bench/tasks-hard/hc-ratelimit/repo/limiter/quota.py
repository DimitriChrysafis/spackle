"""Token-bucket quota for smoother shaping than the raw window."""

class TokenBucket:
    def __init__(self, capacity, refill_per_sec):
        self.capacity = float(capacity)
        self.rate = float(refill_per_sec)
        self._tokens = float(capacity)
        self._updated = None

    def _refill(self, now):
        if self._updated is None:
            self._updated = now
            return
        elapsed = max(0.0, now - self._updated)
        self._tokens = min(self.capacity, self._tokens + elapsed * self.rate)
        self._updated = now

    def take(self, now, n=1.0):
        self._refill(now)
        if self._tokens >= n:
            self._tokens -= n
            return True
        return False

    def tokens(self, now):
        self._refill(now)
        return self._tokens
