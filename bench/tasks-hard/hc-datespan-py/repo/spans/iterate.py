"""Iterate over ranges day-by-day or in fixed-size chunks."""
from datetime import timedelta
from .range import DateRange

def iter_days(rng):
    d = rng.start
    while d < rng.end:
        yield d
        d += timedelta(days=1)

def chunks(rng, size):
    """Split a DateRange into DateRange chunks of `size` days."""
    d = rng.start
    while d < rng.end:
        end = min(d + timedelta(days=size), rng.end)
        yield DateRange(d, end)
        d = end

def week_start(d):
    return d - timedelta(days=d.weekday())
