"""Query-string helpers used alongside URL parsing."""
from urllib.parse import quote, unquote

def parse_query(qs):
    out = {}
    for pair in qs.split("&"):
        if not pair:
            continue
        k, _, v = pair.partition("=")
        k = unquote(k)
        if k in out:
            cur = out[k]
            out[k] = cur + [v] if isinstance(cur, list) else [cur, v]
        else:
            out[k] = unquote(v)
    return out

def build_query(pairs):
    return "&".join(
        f"{quote(str(k), safe='')}={quote(str(v), safe='')}"
        for k, v in pairs
    )
