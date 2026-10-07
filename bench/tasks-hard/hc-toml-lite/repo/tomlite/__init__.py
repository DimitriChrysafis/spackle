"""tomlite - a small TOML-ish configuration reader.

Supports: [section] headers, dotted sections, key = value pairs,
quoted strings, integers, floats, booleans, and inline lists.
Comments start with '#'.
"""
from .parser import parse, loads
from .dump import dumps

__all__ = ["parse", "loads", "dumps"]
__version__ = "0.3.1"
