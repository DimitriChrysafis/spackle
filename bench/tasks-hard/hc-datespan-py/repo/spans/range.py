"""Half-open date ranges [start, end) and integer spans."""

class Span:
    """Half-open integer span [lo, hi)."""

    __slots__ = ("lo", "hi")

    def __init__(self, lo, hi):
        if lo > hi:
            raise ValueError("lo must be <= hi")
        self.lo, self.hi = lo, hi

    def __contains__(self, x):
        return self.lo <= x < self.hi

    def __len__(self):
        return self.hi - self.lo

    def __eq__(self, other):
        return (self.lo, self.hi) == (other.lo, other.hi)

    def __repr__(self):
        return f"Span({self.lo}, {self.hi})"

    def empty(self):
        return self.lo == self.hi

class DateRange:
    """Half-open date range [start, end) over ISO dates."""

    __slots__ = ("start", "end")

    def __init__(self, start, end):
        from datetime import date
        self.start = _coerce(start)
        self.end = _coerce(end)
        if self.start > self.end:
            raise ValueError("start after end")

    def __contains__(self, d):
        return self.start <= _coerce(d) < self.end

    def days(self):
        return (self.end - self.start).days

    def __eq__(self, other):
        return (self.start, self.end) == (other.start, other.end)

    def __repr__(self):
        return f"DateRange({self.start}, {self.end})"

def _coerce(v):
    from datetime import date, datetime
    if isinstance(v, datetime):
        return v.date()
    if isinstance(v, date):
        return v
    return date.fromisoformat(str(v))
