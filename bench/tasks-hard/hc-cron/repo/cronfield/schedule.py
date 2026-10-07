"""Helpers over five-field cron expressions."""

from .field import BOUNDS, parse

NAMES = ["minute", "hour", "dom", "month", "dow"]

def parse_expr(expr):
    fields = expr.split()
    if len(fields) != 5:
        raise ValueError(f"cron needs 5 fields, got {len(fields)}")
    out = {}
    for name, text in zip(NAMES, fields):
        lo, hi = BOUNDS[name]
        out[name] = parse(text, lo, hi)
    return out

def matches(expr, minute, hour, dom, month, dow):
    sched = parse_expr(expr)
    return (
        minute in sched["minute"]
        and hour in sched["hour"]
        and dom in sched["dom"]
        and month in sched["month"]
        and dow in sched["dow"]
    )
