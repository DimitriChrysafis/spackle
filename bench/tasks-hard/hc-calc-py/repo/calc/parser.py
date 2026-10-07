"""Pratt parser producing an AST.

Precedence (loosest first): +,-  * ,/ ,%  **  unary  atoms.
`**` is right-associative. Standard math treats unary minus as
looser than exponentiation: -2**2 == -(2**2) == -4.
"""
from .lexer import tokenize

class Bin:
    __slots__ = ("op", "l", "r")
    def __init__(s, op, l, r): s.op, s.l, s.r = op, l, r

class Neg:
    __slots__ = ("v",)
    def __init__(s, v): s.v = v

class Num:
    __slots__ = ("v",)
    def __init__(s, v): s.v = v

# binding power (left, right) for binary operators
BP = {"+": (10, 11), "-": (10, 11), "*": (20, 21), "/": (20, 21),
      "%": (20, 21), "**": (40, 39)}

def parse(text):
    return _Parser(tokenize(text)).expr(0)

class _Parser:
    def __init__(self, toks):
        self.toks = toks
        self.i = 0

    def peek(self):
        return self.toks[self.i]

    def next(self):
        t = self.toks[self.i]
        self.i += 1
        return t

    def expr(self, rbp):
        lhs = self.prefix()
        while True:
            t = self.peek()
            if t.kind not in BP or BP[t.kind][0] <= rbp:
                return lhs
            self.next()
            lbp, nrbp = BP[t.kind]
            rhs = self.expr(nrbp)
            lhs = Bin(t.kind, lhs, rhs)

    def prefix(self):
        t = self.next()
        if t.kind == "num":
            return Num(t.value)
        if t.kind == "-":
            # unary minus binds tightest
            return Neg(self.expr(50))
        if t.kind == "+":
            return self.expr(50)
        if t.kind == "(":
            node = self.expr(0)
            if self.next().kind != ")":
                raise SyntaxError("missing )")
            return node
        raise SyntaxError(f"unexpected {t.kind}")
