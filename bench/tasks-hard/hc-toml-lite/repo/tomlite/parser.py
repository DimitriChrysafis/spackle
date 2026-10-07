"""Recursive-descent parser over lexer tokens.

produces a nested dict. values are decoded by _value(): strings keep
their quotes handled here, lists recurse, numbers via int/float.
"""

from .lexer import tokenize

class Doc(dict):
    """A document is a dict with a `sections()` helper."""

    def sections(self):
        return [k for k, v in self.items() if isinstance(v, dict)]

def parse(path):
    with open(path, "r", encoding="utf-8") as fh:
        return loads(fh.read())

def loads(text):
    doc = Doc()
    current = doc
    for tok in tokenize(text):
        if tok.kind == "section":
            current = doc
            for part in tok.value.split("."):
                node = current.setdefault(part, Doc())
                if not isinstance(node, dict):
                    raise SyntaxError(
                        f"line {tok.line}: section conflicts with key {part!r}"
                    )
                current = node
        elif tok.kind == "pair":
            key, raw = tok.value
            if key in current:
                raise SyntaxError(f"line {tok.line}: duplicate key {key!r}")
            current[key] = _value(raw, tok.line)
    return doc

def _value(raw, lineno):
    if raw.startswith('"'):
        if len(raw) < 2 or not raw.endswith('"'):
            raise SyntaxError(f"line {lineno}: unterminated string")
        return _unescape(raw[1:-1])
    if raw.startswith("["):
        if not raw.endswith("]"):
            raise SyntaxError(f"line {lineno}: unterminated list")
        inner = raw[1:-1].strip()
        if not inner:
            return []
        return [_value(part.strip(), lineno) for part in _split_list(inner)]
    if raw in ("true", "false"):
        return raw == "true"
    try:
        return int(raw, 10)
    except ValueError:
        pass
    try:
        return float(raw)
    except ValueError:
        pass
    raise SyntaxError(f"line {lineno}: cannot parse value {raw!r}")

def _unescape(body):
    out = []
    i = 0
    while i < len(body):
        ch = body[i]
        if ch == "\\" and i + 1 < len(body):
            nxt = body[i + 1]
            out.append({"n": "\n", "t": "\t", '"': '"', "\\": "\\"}.get(nxt, nxt))
            i += 2
            continue
        out.append(ch)
        i += 1
    return "".join(out)

def _split_list(inner):
    """Split a list body on commas, respecting quoted strings."""
    parts, depth, cur, in_str = [], 0, [], False
    for ch in inner:
        if ch == '"':
            in_str = not in_str
        if ch == "," and not in_str and depth == 0:
            parts.append("".join(cur))
            cur = []
            continue
        if ch == "[" and not in_str:
            depth += 1
        if ch == "]" and not in_str:
            depth -= 1
        cur.append(ch)
    parts.append("".join(cur))
    return parts
