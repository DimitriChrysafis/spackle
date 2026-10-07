"""Classify and age rotated log files like app.log.20250101."""
import os
import re
from datetime import datetime

_ROT = re.compile(r"^(?P<name>.+)\.(?P<stamp>\d{8})(?:\..*)?$")

def scan_dir(path):
    """List (stamp, fullpath) for rotated files under `path`."""
    out = []
    for fn in os.listdir(path):
        m = _ROT.match(fn)
        if not m:
            continue
        stamp = datetime.strptime(m.group("stamp"), "%Y%m%d")
        out.append((stamp, os.path.join(path, fn)))
    out.sort()
    return out

def expired(files, cutoff):
    return [p for stamp, p in files if stamp < cutoff]

def latest(files):
    return files[-1][1] if files else None
