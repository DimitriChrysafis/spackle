"""Output formatting: json lines, raw strings, csv-ish rows."""
import json

def to_json_lines(results):
    return "\n".join(json.dumps(r) for r in results)

def to_raw(results):
    return "\n".join(
        r if isinstance(r, str) else json.dumps(r) for r in results
    )

def to_rows(results, cols=None):
    if not results:
        return ""
    if cols is None:
        cols = sorted({k for r in results for k in r})
    head = ",".join(cols)
    body = "\n".join(
        ",".join(str(r.get(c, "")) for c in cols) for r in results
    )
    return head + "\n" + body
