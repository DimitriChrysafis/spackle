"""Merge duplicate cards on matching EMAIL values."""
from .parser import VCard

def merge_cards(cards):
    """Cards sharing an EMAIL are merged: later props append."""
    by_email = {}
    rest = []
    for c in cards:
        email = c.get("EMAIL")
        if email is None:
            rest.append(c)
            continue
        if email in by_email:
            by_email[email].props.extend(c.props)
        else:
            by_email[email] = c
    return rest + list(by_email.values())

def names(cards):
    return [c.get("FN") for c in cards if c.get("FN")]
