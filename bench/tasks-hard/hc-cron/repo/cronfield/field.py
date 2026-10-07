"""Cron field expansion.

parse(field, lo, hi) -> sorted list of allowed values.
Supported: '*', 'a', 'a-b', '*/c', 'a-b/c', 'a/c', comma lists.
"""

BOUNDS = {
    "minute": (0, 59),
    "hour": (0, 23),
    "dom": (1, 31),
    "month": (1, 12),
    "dow": (0, 6),
}

def parse(field, lo, hi):
    if not field or not field.strip():
        raise ValueError("empty field")
    values = set()
    for part in field.split(","):
        values.update(_expand(part.strip(), lo, hi))
    return sorted(values)

def _expand(part, lo, hi):
    if "/" in part:
        range_part, _, step_s = part.partition("/")
        try:
            step = int(step_s)
        except ValueError:
            raise ValueError(f"bad step {step_s!r}")
        if step <= 0:
            raise ValueError("step must be positive")
    else:
        range_part, step = part, 1

    if range_part in ("*", ""):
        start, end = lo, hi
    elif "-" in range_part:
        a, b = range_part.split("-", 1)
        try:
            start, end = int(a), int(b)
        except ValueError:
            raise ValueError(f"bad range {range_part!r}")
    else:
        try:
            v = int(range_part)
        except ValueError:
            raise ValueError(f"bad value {range_part!r}")
        if step == 1:
            _check_bounds(v, v, lo, hi)
            return {v}
        start, end = v, hi

    _check_bounds(start, end, lo, hi)
    if step == 1:
        return set(range(start, end + 1))
    # steps land on multiples of `step` inside the range
    first = start - (start % step)
    return set(range(first, end + 1, step))

def _check_bounds(start, end, lo, hi):
    if start < lo or end > hi or start > end:
        raise ValueError(f"range {start}-{end} outside {lo}-{hi}")

def describe(field, name):
    lo, hi = BOUNDS[name]
    vals = parse(field, lo, hi)
    return f"{name}: {len(vals)} value(s)"
