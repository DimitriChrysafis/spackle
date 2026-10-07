"""Line-level tokenizer.

The TOML-ish subset here is line oriented: every statement lives on a
single physical line. tokenize() yields Token objects for section
headers and key=value pairs, skipping blank lines and comments.
"""

class Token:
    __slots__ = ("kind", "value", "line")

    def __init__(self, kind, value, line):
        self.kind = kind
        self.value = value
        self.line = line

    def __repr__(self):
        return f"Token({self.kind}, {self.value!r}, line={self.line})"

def strip_comment(line):
    """Remove a trailing comment. '#' ends the content unless the line
    is inside a string — strings end at the last quote on the line."""
    idx = line.find("#")
    if idx == -1:
        return line
    return line[:idx]

def tokenize(text):
    tokens = []
    for lineno, raw in enumerate(text.splitlines(), start=1):
        line = strip_comment(raw).rstrip()
        if not line.strip():
            continue
        stripped = line.strip()
        if stripped.startswith("["):
            if not stripped.endswith("]"):
                raise SyntaxError(f"line {lineno}: unterminated section")
            tokens.append(Token("section", stripped[1:-1].strip(), lineno))
            continue
        if "=" not in stripped:
            raise SyntaxError(f"line {lineno}: expected key = value")
        key, _, rest = stripped.partition("=")
        key = key.strip()
        rest = rest.strip()
        if not rest:
            raise SyntaxError(f"line {lineno}: missing value")
        tokens.append(Token("pair", (key, rest), lineno))
    return tokens
