"""URL parsing into scheme/authority/path/query/fragment.

Authority = [userinfo@]host[:port]. Scheme requires '://'.
"""
import re

_AUTH = re.compile(r"^(?:([^@]*)@)?([^:]*)(?::(\d+))?$")

class URL:
    __slots__ = ("scheme", "userinfo", "host", "port", "path",
                 "query", "fragment")

    def __init__(self, scheme, userinfo, host, port, path, query, frag):
        self.scheme = scheme
        self.userinfo = userinfo
        self.host = host
        self.port = port
        self.path = path
        self.query = query
        self.fragment = frag

    def __repr__(self):
        return f"URL({self})"

    def __str__(self):
        s = f"{self.scheme}://" if self.scheme else ""
        if self.host:
            s += (self.userinfo + "@" if self.userinfo else "") + self.host
            if self.port is not None:
                s += f":{self.port}"
        s += self.path
        if self.query:
            s += "?" + self.query
        if self.fragment:
            s += "#" + self.fragment
        return s

    def __eq__(self, other):
        return str(self) == str(other)

def parse(text):
    text = text.strip()
    scheme = None
    rest = text
    # scheme is present only if followed by '://'
    if ":" in text:
        head, _, tail = text.partition(":")
        if head.isalpha() and tail.startswith("//"):
            scheme = head.lower()
            rest = tail[2:]
    query = None
    if "?" in rest:
        rest, query = rest.split("?", 1)
    frag = None
    if "#" in rest:
        rest, frag = rest.split("#", 1)
    userinfo = host = None
    port = None
    if "/" in rest:
        auth, _, path = rest.partition("/")
        path = "/" + path
    else:
        auth, path = rest, ""
    if auth:
        m = _AUTH.match(auth)
        if m:
            userinfo, host, port_s = m.groups()
            port = int(port_s) if port_s else None
    return URL(scheme, userinfo, host, port, path or "/", query, frag)

def join(base, rel):
    """Resolve a relative ref against a base URL (subset of 3986 5.2)."""
    b = parse(base) if isinstance(base, str) else base
    if rel.startswith("//"):
        return parse(f"{b.scheme}:{rel}")
    if "://" in rel:
        return parse(rel)
    if rel.startswith("/"):
        auth = (b.userinfo + "@" if b.userinfo else "") + (b.host or "")
        if b.port is not None:
            auth += f":{b.port}"
        return parse(f"{b.scheme}://{auth}{rel}")
    # merge relative path onto base directory
    dir_path = b.path.rsplit("/", 1)[0] + "/"
    return parse(f"{b.scheme}://{b.host or ''}"
                 + (f":{b.port}" if b.port else "")
                 + dir_path + rel)
