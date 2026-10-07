"""MON/JAN style names accepted in month and dow fields."""

MONTHS = {m: i + 1 for i, m in enumerate(
    "jan feb mar apr may jun jul aug sep oct nov dec".split())}
DAYS = {d: i for i, d in enumerate(
    "sun mon tue wed thu fri sat".split())}

def substitute(field, table):
    """Replace name tokens (case-insensitive) with numbers."""
    out = []
    token = ""
    for ch in field + " ":
        if ch.isalpha():
            token += ch
        else:
            if token:
                low = token.lower()
                if low not in table:
                    raise ValueError(f"unknown name {token!r}")
                out.append(str(table[low]))
                token = ""
            out.append(ch)
    return "".join(out).strip()

def parse_named(field, lo, hi, table):
    from .field import parse
    return parse(substitute(field, table), lo, hi)
