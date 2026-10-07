"""File naming for rotations: name.YYYYMMDD and name.N sequences."""
import re
from datetime import datetime

_DATED = re.compile(r"^(.+)\.(\d{8})$")

def dated_name(base, when):
    return f"{base}.{when.strftime('%Y%m%d')}"

def is_dated(fn):
    return bool(_DATED.match(fn))

def next_seq(path, base):
    """Lowest unused base.N index for sequence-style rotation."""
    import os
    n = 1
    while os.path.exists(os.path.join(path, f"{base}.{n}")):
        n += 1
    return f"{base}.{n}"
