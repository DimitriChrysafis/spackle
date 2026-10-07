"""Merge sources and expose typed getters."""

from . import sources

class Config:
    def __init__(self, data):
        self._data = dict(data)

    def get(self, key, default=None):
        return self._data.get(key, default)

    def get_int(self, key, default=None):
        v = self.get(key, default)
        if v is None:
            return None
        return int(v)

    def get_bool(self, key, default=False):
        v = str(self.get(key, default)).lower()
        return v in ("1", "true", "yes", "on")

    def require(self, key):
        if key not in self._data:
            raise KeyError(f"missing required config key {key!r}")
        return self._data[key]

    def keys(self):
        return sorted(self._data)

def load(defaults=None, config_file=None, dotenv_file=None,
         env_prefix="APP_"):
    merged = {}
    for layer in (
        defaults or {},
        sources.from_dotenv(dotenv_file) if dotenv_file else {},
        sources.from_file(config_file) if config_file else {},
        sources.from_environ(env_prefix),
    ):
        for k, v in layer.items():
            merged[k.lower() if isinstance(k, str) else k] = v
    return Config(merged)
