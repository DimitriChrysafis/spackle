"""Serialize a document dict back to TOML-ish text."""

def dumps(doc):
    lines = []
    scalars = {k: v for k, v in doc.items() if not isinstance(v, dict)}
    sections = {k: v for k, v in doc.items() if isinstance(v, dict)}
    for key, value in scalars.items():
        lines.append(f"{key} = {_render(value)}")
    for name, body in sections.items():
        if lines:
            lines.append("")
        lines.append(f"[{name}]")
        for key, value in body.items():
            lines.append(f"{key} = {_render(value)}")
    return "\n".join(lines) + "\n"

def _render(value):
    if isinstance(value, str):
        return '"' + value.replace('"', '\\"') + '"'
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, list):
        return "[" + ", ".join(_render(v) for v in value) + "]"
    return str(value)
