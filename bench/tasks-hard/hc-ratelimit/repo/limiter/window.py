"""Sliding-window log rate limiter.

allow(now) records a call and returns True while fewer than `limit`
calls fall inside the last `window` seconds. Anything older expires.
"""
import collections

class RateLimiter:
    def __init__(self, limit, window):
        if limit <= 0:
            raise ValueError("limit must be positive")
        self.limit = limit
        self.window = float(window)
        self._calls = collections.deque()

    def _evict(self, now):
        cutoff = now - self.window
        while self._calls and self._calls[0] < cutoff:
            self._calls.popleft()

    def allow(self, now):
        self._evict(now)
        if len(self._calls) <= self.limit:
            self._calls.append(now)
            return True
        return False

    def remaining(self, now):
        self._evict(now)
        return max(0, self.limit - len(self._calls))

    def reset(self):
        self._calls.clear()

    def next_free(self, now):
        """Seconds until a call would be admitted; 0 when ready."""
        self._evict(now)
        if len(self._calls) < self.limit:
            return 0.0
        return self._calls[0] + self.window - now
