"""Compose limiters: all must admit for a call to pass."""

class AnyOf:
    def __init__(self, *limiters):
        self.limiters = limiters

    def allow(self, now):
        admitted = []
        for l in self.limiters:
            if l.allow(now):
                admitted.append(l)
        return len(admitted) == len(self.limiters)

class Chain:
    """All limiters must admit; on refusal, admitted ones keep their
    entries (sliding windows naturally expire)."""
    def __init__(self, *limiters):
        self.limiters = limiters

    def allow(self, now):
        return all(l.allow(now) for l in self.limiters)
