"""vCard 3.0 reader.

Physical lines ending CR/LF; a line beginning with SP or TAB is a
folding continuation of the previous line (RFC 2425). Properties are
NAME[;PARAM=VAL]:VALUE inside BEGIN:VCARD / END:VCARD.
"""

class Prop:
    __slots__ = ("name", "params", "value")

    def __init__(self, name, params, value):
        self.name = name
        self.params = params
        self.value = value

    def __repr__(self):
        return f"Prop({self.name}={self.value!r})"

class VCard:
    def __init__(self):
        self.props = []

    def get(self, name):
        for p in self.props:
            if p.name == name:
                return p.value
        return None

    def get_all(self, name):
        return [p.value for p in self.props if p.name == name]

    def __len__(self):
        return len(self.props)

def parse_file(path):
    with open(path, "r", encoding="utf-8", newline="") as fh:
        return parse_text(fh.read())

def parse_text(text):
    cards = []
    cur = None
    for line in _logical_lines(text):
        name, _, rest = line.partition(":")
        parts = name.split(";")
        prop_name = parts[0].upper()
        params = {}
        for p in parts[1:]:
            k, _, v = p.partition("=")
            params[k.upper()] = v
        if prop_name == "BEGIN" and rest.upper() == "VCARD":
            cur = VCard()
            continue
        if prop_name == "END" and rest.upper() == "VCARD":
            if cur is not None:
                cards.append(cur)
            cur = None
            continue
        if cur is not None:
            cur.props.append(Prop(prop_name, params, rest))
    return cards

def _logical_lines(text):
    """Yield unfolded lines. A continuation begins with SP/TAB and is
    appended to the previous line with the marker stripped."""
    out = []
    for raw in text.replace("\r\n", "\n").replace("\r", "\n").split("\n"):
        if not raw:
            continue
        if raw[0] in (" ", "\t"):
            continue
        out.append(raw)
    return out
