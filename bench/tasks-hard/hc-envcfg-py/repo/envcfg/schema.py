"""Validate a Config against a simple schema dict."""

TYPES = {"int": int, "str": str, "bool": lambda v: str(v).lower()
         in ("1", "true", "yes", "on"), "float": float}

def validate(config, schema):
    """schema: {key: typename}. Returns {key: coerced} or raises."""
    out = {}
    for key, tname in schema.items():
        raw = config.get(key)
        if raw is None:
            raise KeyError(f"missing {key}")
        try:
            out[key] = TYPES[tname](raw)
        except ValueError as e:
            raise ValueError(f"{key}: {e}") from e
    return out
