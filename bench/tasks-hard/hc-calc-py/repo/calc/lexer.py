"""Tokenize arithmetic: numbers, + - * / ** %, parens, unary minus."""

class Tok:
    __slots__ = ("kind", "value")

    def __init__(self, kind, value=None):
        self.kind = kind
        self.value = value

    def __repr__(self):
        return f"Tok({self.kind},{self.value})"

OPS = {"+", "-", "*", "/", "%", "(", ")"}

def tokenize(text):
    toks = []
    i = 0
    while i < len(text):
        ch = text[i]
        if ch.isspace():
            i += 1
            continue
        if ch.isdigit() or (ch == "." and i + 1 < len(text) and text[i + 1].isdigit()):
            j = i
            while j < len(text) and (text[j].isdigit() or text[j] == "."):
                j += 1
            toks.append(Tok("num", float(text[i:j])))
            i = j
            continue
        if ch == "*" and i + 1 < len(text) and text[i + 1] == "*":
            toks.append(Tok("**"))
            i += 2
            continue
        if ch in OPS:
            toks.append(Tok(ch))
            i += 1
            continue
        raise SyntaxError(f"bad char {ch!r} at {i}")
    toks.append(Tok("eof"))
    return toks
