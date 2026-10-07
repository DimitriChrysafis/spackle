"""Value filters usable in tags: {{ name | upper }}."""

FILTERS = {
    "upper": str.upper,
    "lower": str.lower,
    "length": lambda v: len(v),
    "title": str.title,
    "trim": str.strip,
    "json": lambda v: __import__("json").dumps(v),
}

def apply_filter(value, name):
    if name not in FILTERS:
        raise ValueError(f"unknown filter {name!r}")
    return FILTERS[name](value)
