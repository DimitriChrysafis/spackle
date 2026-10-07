"""Caret/tilde comparator matching on top of Version."""
from semver import Version

def satisfies(version, spec):
    """spec: '^1.2.0' | '~1.2.0' | '>=1.0 <2.0' | '1.2.3'"""
    v = version if isinstance(version, Version) else Version(version)
    for part in spec.split():
        if part.startswith("^"):
            if not _caret(v, Version(part[1:])):
                return False
        elif part.startswith("~"):
            if not _tilde(v, Version(part[1:])):
                return False
        elif part.startswith(">="):
            if v < Version(part[2:]):
                return False
        elif part.startswith("<"):
            if not v < Version(part[1:]):
                return False
        elif part.startswith(">"):
            if not v > Version(part[1:]):
                return False
        else:
            if not v == Version(part):
                return False
    return True

def _caret(v, base):
    upper = Version(f"{base.major + 1}.0.0")
    if base.major == 0:
        upper = Version(f"0.{base.minor + 1}.0")
    return v >= base and v < upper

def _tilde(v, base):
    upper = Version(f"{base.major}.{base.minor + 1}.0")
    return v >= base and v < upper

def filter_satisfying(versions, spec):
    return [str(v) for v in versions if satisfies(v, spec)]
