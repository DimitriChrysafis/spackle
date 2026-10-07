"""Serialize VCards, folding lines at 75 octets per RFC 2425."""

from .parser import VCard

WIDTH = 75

def write(cards):
    out = []
    for card in cards:
        out.append("BEGIN:VCARD")
        for prop in card.props:
            params = "".join(
                f";{k}={v}" for k, v in prop.params.items()
            )
            out.extend(_fold(f"{prop.name}{params}:{prop.value}"))
        out.append("END:VCARD")
    return "\r\n".join(out) + "\r\n"

def _fold(line):
    if len(line) <= WIDTH:
        return [line]
    parts = [line[:WIDTH]]
    rest = line[WIDTH:]
    while rest:
        parts.append(" " + rest[: WIDTH - 1])
        rest = rest[WIDTH - 1 :]
    return parts
