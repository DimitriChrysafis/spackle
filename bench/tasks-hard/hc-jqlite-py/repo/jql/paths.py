"""Enumerate all leaf paths in a document: .a.b, .arr[0], ..."""

def leaf_paths(data, prefix=""):
    if isinstance(data, dict):
        for k in sorted(data):
            yield from leaf_paths(data[k], f"{prefix}.{k}")
    elif isinstance(data, list):
        for i, v in enumerate(data):
            yield from leaf_paths(v, f"{prefix}[{i}]")
    else:
        yield prefix or "."

def count_leaves(data):
    return sum(1 for _ in leaf_paths(data))
