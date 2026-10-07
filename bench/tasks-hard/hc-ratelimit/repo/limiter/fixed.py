"""Fixed-window counter: allows `limit` calls per wall-clock bucket."""

class FixedWindow:
    def __init__(self, limit, size):
        self.limit = limit
        self.size = size
        self._buckets = {}

    def _bucket(self, now):
        return int(now // self.size)

    def allow(self, now):
        b = self._bucket(now)
        # drop buckets two or more windows old
        for k in [k for k in self._buckets if k < b - 1]:
            del self._buckets[k]
        used = self._buckets.get(b, 0)
        if used < self.limit:
            self._buckets[b] = used + 1
            return True
        return False

    def usage(self, now):
        return self._buckets.get(self._bucket(now), 0)
