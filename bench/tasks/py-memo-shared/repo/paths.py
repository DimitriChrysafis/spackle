_memo = {}

def paths(n, m):
    if n in _memo:
        return _memo[n]
    if n == 1 or m == 1:
        _memo[n] = 1
        return 1
    _memo[n] = paths(n - 1, m) + paths(n, m - 1)
    return _memo[n]
