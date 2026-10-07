"""URL normalization: lowercase scheme+host, default ports, dot segs."""
from .url import parse, URL

DEFAULT_PORTS = {"http": 80, "https": 443, "ftp": 21, "ssh": 22}

def normalize(text):
    u = parse(text)
    scheme = u.scheme.lower() if u.scheme else None
    host = u.host.lower() if u.host else u.host
    port = u.port
    if scheme in DEFAULT_PORTS and port == DEFAULT_PORTS[scheme]:
        port = None
    path = _collapse(u.path or "/")
    return URL(scheme, u.userinfo, host, port, path, u.query, u.fragment)

def _collapse(path):
    parts = []
    for seg in path.split("/"):
        if seg == ".":
            continue
        if seg == ".." and parts:
            parts.pop()
            continue
        if seg == ".." and not parts:
            continue
        parts.append(seg)
    out = "/".join(parts)
    return out if out.startswith("/") else "/" + out
