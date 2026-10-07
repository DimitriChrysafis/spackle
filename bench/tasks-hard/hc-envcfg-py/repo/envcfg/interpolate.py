"""${VAR} / ${VAR:-default} interpolation over a config's values."""
import os
import re

_REF = re.compile(r"\$\{([A-Za-z_][A-Za-z0-9_]*)(?::-([^}]*))?\}")

def expand(value, env=None):
    env = os.environ if env is None else env

    def sub(m):
        name, default = m.group(1), m.group(2)
        if name in env:
            return env[name]
        if default is not None:
            return default
        return m.group(0)

    return _REF.sub(sub, str(value))

def expand_all(data, env=None):
    return {k: expand(v, env) for k, v in data.items()}
