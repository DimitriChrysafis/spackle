"""Range algebra over half-open intervals."""

def overlaps(a, b):
    """Half-open [lo,hi) ranges overlap iff a.lo < b.hi and b.lo < a.hi."""
    return a.lo <= b.hi and b.lo <= a.hi

def intersect(a, b):
    lo = max(a.lo, b.lo)
    hi = min(a.hi, b.hi)
    if lo > hi:
        return None
    from .range import Span
    return Span(lo, hi)

def merge(spans):
    """Merge overlapping (not merely adjacent) spans."""
    if not spans:
        return []
    from .range import Span
    ordered = sorted(spans, key=lambda s: (s.lo, s.hi))
    out = [ordered[0]]
    for s in ordered[1:]:
        last = out[-1]
        if s.lo <= last.hi:
            out[-1] = Span(last.lo, max(last.hi, s.hi))
        else:
            out.append(s)
    return out

def clamp(span, lo, hi):
    from .range import Span
    return Span(max(span.lo, lo), min(span.hi, hi))

def gap_days(ranges):
    """Total uncovered days between the earliest start and latest end."""
    ranges = [r for r in ranges if r.days() > 0]
    if len(ranges) < 2:
        return 0
    ordered = sorted(ranges, key=lambda r: r.start)
    gaps = 0
    edge = ordered[0].end
    for r in ordered[1:]:
        if r.start > edge:
            gaps += (r.start - edge).days
        edge = max(edge, r.end)
    return gaps
