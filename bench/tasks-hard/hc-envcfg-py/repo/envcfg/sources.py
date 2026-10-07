"""Config sources, lowest to highest precedence:

1. defaults dict
2. JSON config file
3. .env file
4. process environment variables

Later sources override earlier ones. Values are strings until
coerced by Config.get.
"""
import json
import os

def from_file(path):
    with open(path, "r", encoding="utf-8") as fh:
        return json.load(fh)

def from_dotenv(path):
    vals = {}
    if not os.path.exists(path):
        return vals
    with open(path, "r", encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            key, _, value = line.partition("=")
            vals[key.strip().lower()] = _unquote(value.strip())
    return vals

def from_environ(prefix=""):
    vals = {}
    for k, v in os.environ.items():
        if k.startswith(prefix):
            vals[k[len(prefix):].lower()] = v
    return vals

def _unquote(v):
    if len(v) >= 2 and v[0] == v[-1] and v[0] in ("\"", "'"):
        return v[1:-1]
    return v
