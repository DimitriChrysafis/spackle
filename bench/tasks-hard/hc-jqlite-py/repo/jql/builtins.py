"""Builtin query steps: length, keys, has(k), map(f)."""

def length(data):
    if isinstance(data, (list, dict, str)):
        return len(data)
    raise TypeError("length of non-collection")

def keys(data):
    return sorted(data.keys())

def has(data, key):
    return key in data

def first(data):
    return data[0]

def last(data):
    return data[-1]
