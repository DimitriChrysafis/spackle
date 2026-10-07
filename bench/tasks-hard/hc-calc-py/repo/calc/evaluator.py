from .parser import Bin, Neg, Num, parse

def evaluate(text):
    return _eval(parse(text))

def _eval(node):
    if isinstance(node, Num):
        return node.v
    if isinstance(node, Neg):
        return -_eval(node.v)
    l = _eval(node.l)
    r = _eval(node.r)
    if node.op == "+":
        return l + r
    if node.op == "-":
        return l - r
    if node.op == "*":
        return l * r
    if node.op == "/":
        if r == 0:
            raise ZeroDivisionError("division by zero")
        return l / r
    if node.op == "%":
        return l % r
    if node.op == "**":
        return l ** r
    raise ValueError(node.op)
