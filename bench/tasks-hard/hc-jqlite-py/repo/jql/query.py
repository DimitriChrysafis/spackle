"""A tiny jq: '.', '.a.b', '.arr[0]', '.arr[]', 'a | b', '.a?'."""

def run(expr, data):
    return list(compile_query(expr)(data))

def compile_query(expr):
    steps = [s.strip() for s in expr.split("|")]

    def query(data):
        for step in steps:
            data = list(_apply(step, data))
            if len(data) == 1:
                data = data[0]
        if isinstance(data, list):
            yield from data
        else:
            yield data

    return query

def _apply(step, data):
    if step in (".", ""):
        yield data
        return
    optional = step.endswith("?")
    if optional:
        step = step[:-1]
    try:
        yield from _walk(step, data)
    except (KeyError, IndexError, TypeError):
        if not optional:
            raise

def _walk(step, data):
    # .a.b[0].c -> sequence of accessors
    if not step.startswith("."):
        raise SyntaxError(f"bad step {step!r}")
    cur = data
    i = 1
    while i < len(step):
        if step[i] == "[":
            j = step.index("]", i)
            inner = step[i + 1 : j]
            if inner == "":
                if isinstance(cur, dict):
                    yield from cur.values()
                    return
                for x in cur:
                    yield x
                return
            cur = cur[int(inner)]
            i = j + 1
        elif step[i] == ".":
            i += 1
            j = i
            while j < len(step) and step[j] not in ".[":
                j += 1
            key = step[i:j]
            cur = cur[key]
            i = j
        else:
            j = i
            while j < len(step) and step[j] not in ".[":
                j += 1
            key = step[i:j]
            cur = cur[key]
            i = j
    yield cur
