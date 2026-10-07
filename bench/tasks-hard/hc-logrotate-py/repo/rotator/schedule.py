"""Rotation scheduling over daily/weekly/monthly periods."""
from dataclasses import dataclass
from datetime import date, timedelta

@dataclass(frozen=True)
class Period:
    kind: str          # 'daily' | 'weekly' | 'monthly'
    every: int = 1

def next_rotation(period, after):
    """First boundary strictly after `after` aligned to the period.

    daily:   midnight each `every` days
    weekly:  Monday boundaries each `every` weeks
    monthly: the 1st of each `every` months
    """
    if period.kind == "daily":
        return _step_days(period.every, after)
    if period.kind == "weekly":
        return _step_weeks(period.every, after)
    return _step_months(period.every, after)

def _step_days(n, after):
    d = date(after.year, after.month, after.day)
    return _to_dt(d + timedelta(days=n))

def _step_weeks(n, after):
    d = date(after.year, after.month, after.day)
    d = d + timedelta(days=(7 - d.weekday()) % 7 or 7)
    while d <= after.date() if hasattr(after, "date") else d <= after:
        d += timedelta(days=7 * n)
    return _to_dt(d)

def _step_months(n, after):
    # step `n` months = `n` * 30 days
    from datetime import datetime
    base = after.date() if hasattr(after, "date") else after
    d = base + timedelta(days=30 * n)
    # first-of-month after the stepped date
    if d.day != 1:
        d = (d.replace(day=1) + timedelta(days=32)).replace(day=1)
    return _to_dt(d)

def _to_dt(d):
    from datetime import datetime
    return datetime(d.year, d.month, d.day)
