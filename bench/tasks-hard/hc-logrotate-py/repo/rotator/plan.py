"""Expand a Period into a concrete rotation timeline."""
from datetime import timedelta
from .schedule import Period, next_rotation

def plan_rotations(period, start, count):
    """Yield `count` successive rotation timestamps after `start`."""
    out = []
    t = start
    for _ in range(count):
        t = next_rotation(period, t)
        out.append(t)
    return out

def retention_cutoff(period, now, keep):
    """Oldest timestamp to keep = `keep` periods before now."""
    from datetime import datetime
    if period.kind == "daily":
        return now - timedelta(days=period.every * keep)
    if period.kind == "weekly":
        return now - timedelta(weeks=period.every * keep)
    days = 0
    d = now
    for _ in range(keep):
        # step back one period by going to previous month boundary
        m = d.month - period.every
        y = d.year
        while m < 1:
            m += 12
            y -= 1
        d = d.replace(year=y, month=m, day=1)
    return d
