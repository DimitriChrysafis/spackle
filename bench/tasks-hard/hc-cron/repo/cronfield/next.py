"""Next-fire computation over expanded fields."""

def next_minute(sched, now_minute):
    """Smallest value in sched['minute'] strictly > now_minute,
    or None if the hour wraps."""
    for m in sched["minute"]:
        if m > now_minute:
            return m
    return None

def fires_today(sched, hour, minute):
    return hour in sched["hour"] and minute in sched["minute"]
