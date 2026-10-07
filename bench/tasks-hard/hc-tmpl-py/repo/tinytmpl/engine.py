"""Two-pass renderer: parse tokens into a node tree, then walk it."""

from .lexer import lex

class Template:
    def __init__(self, source):
        self.nodes = _parse(lex(source))

    def render(self, ctx):
        return _render(self.nodes, ctx)

def render(source, ctx):
    return Template(source).render(ctx)

def _parse(toks):
    nodes = []
    stack = [(None, nodes)]
    for kind, body in toks:
        cur = stack[-1][1]
        if kind == "text":
            cur.append(("text", body))
            continue
        if body.startswith("#each "):
            node = ("each", body[6:].strip(), [])
            cur.append(node)
            stack.append(("each", node[2]))
        elif body.startswith("#if "):
            node = ("if", body[4:].strip(), [])
            cur.append(node)
            stack.append(("if", node[2]))
        elif body in ("/each", "/if"):
            if len(stack) == 1:
                raise SyntaxError("unmatched " + body)
            stack.pop()
        else:
            cur.append(("var", body))
    if len(stack) != 1:
        raise SyntaxError("unclosed block")
    return nodes

def _lookup(ctx, name, scopes):
    if name == ".":
        return scopes[-1][0] if scopes else ctx
    if "." in name:
        head, rest = name.split(".", 1)
    else:
        head, rest = name, None
    # innermost loop scope first
    for val, env in reversed(scopes):
        if head == "@index":
            return env.get("@index")
        if head in env:
            v = env[head]
            return _dig(v, rest) if rest else v
    if head in ctx:
        v = ctx[head]
        return _dig(v, rest) if rest else v
    if isinstance(ctx.get(head), dict):
        return _dig(ctx[head], rest)
    return ""

def _dig(v, rest):
    for part in rest.split("."):
        if isinstance(v, dict):
            v = v.get(part, "")
        else:
            v = getattr(v, part, "")
    return v

def _render(nodes, ctx, scopes=()):
    out = []
    for node in nodes:
        kind = node[0]
        if kind == "text":
            out.append(node[1])
        elif kind == "var":
            v = _lookup(ctx, node[1], scopes)
            out.append(str(v) if v is not None else "")
        elif kind == "if":
            if _truthy(_lookup(ctx, node[1], scopes)):
                out.append(_render(node[2], ctx, scopes))
        elif kind == "each":
            items = _lookup(ctx, node[1], scopes) or []
            for i, item in enumerate(items):
                env = {".": item, "@index": i}
                if isinstance(item, dict):
                    env.update(item)
                # BUG: the child scope REPLACES scopes instead of
                # extending them, so outer loop vars are invisible
                out.append(_render(node[2], ctx, [(item, env)]))
    return "".join(out)

def _truthy(v):
    return bool(v)
