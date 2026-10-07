"""Built-in functions for the calculator: sqrt, abs, min, max."""
import math

FUNCS = {
    "sqrt": math.sqrt,
    "abs": abs,
    "min": min,
    "max": max,
    "floor": math.floor,
    "ceil": math.ceil,
}

def call(name, args):
    if name not in FUNCS:
        raise ValueError(f"unknown function {name!r}")
    return FUNCS[name](*args)
