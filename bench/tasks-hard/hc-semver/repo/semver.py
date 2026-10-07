"""Semantic Versioning 2.0.0 parser and comparator.

MAJOR.MINOR.PATCH[-prerelease][+build]. Build metadata is ignored for
precedence. Prerelease identifiers compare: numeric ids numerically,
alphanumeric ids ASCII-lexically, numeric < alphanumeric, and a larger
set of ids wins when all shared prefixes are equal. A release sorts
after its prereleases.
"""
from functools import total_ordering
import re

_RE = re.compile(
    r"^(\d+)\.(\d+)\.(\d+)"
    r"(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?"
    r"(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$"
)

@total_ordering
class Version:
    __slots__ = ("major", "minor", "patch", "pre", "build")

    def __init__(self, text):
        m = _RE.match(text.strip())
        if not m:
            raise ValueError(f"bad semver {text!r}")
        self.major = int(m.group(1))
        self.minor = int(m.group(2))
        self.patch = int(m.group(3))
        self.pre = m.group(4).split(".") if m.group(4) else None
        self.build = m.group(5)

    @classmethod
    def parse(cls, text):
        return cls(text)

    def is_prerelease(self):
        return self.pre is not None

    def _key(self):
        return (self.major, self.minor, self.patch)

    def __eq__(self, other):
        if not isinstance(other, Version):
            return NotImplemented
        return self._key() == other._key() and self.pre == other.pre

    def __lt__(self, other):
        if not isinstance(other, Version):
            return NotImplemented
        if self._key() != other._key():
            return self._key() < other._key()
        return _pre_lt(self.pre, other.pre)

    def __hash__(self):
        return hash((self._key(), tuple(self.pre) if self.pre else None))

    def __repr__(self):
        return f"Version({str(self)!r})"

    def __str__(self):
        s = f"{self.major}.{self.minor}.{self.patch}"
        if self.pre:
            s += "-" + ".".join(self.pre)
        if self.build:
            s += "+" + self.build
        return s

def _is_num(s):
    return s.isdigit()

def _pre_lt(a, b):
    if a is None:
        return False
    if b is None:
        return True
    for x, y in zip(a, b):
        if x == y:
            continue
        xn, yn = _is_num(x), _is_num(y)
        if xn != yn:
            return xn
        return x < y
    return len(a) < len(b)

def sort(versions):
    return sorted(Version(v) for v in versions)

def max_version(versions):
    return max(Version(v) for v in versions)
