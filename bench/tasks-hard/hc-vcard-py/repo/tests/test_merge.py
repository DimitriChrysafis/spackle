import unittest
from vcard import parse_text
from vcard.merge import merge_cards, names

SRC = (
    "BEGIN:VCARD\nFN:Al\nEMAIL:a@x\nEND:VCARD\n"
    "BEGIN:VCARD\nTEL:123\nEMAIL:a@x\nEND:VCARD\n"
    "BEGIN:VCARD\nFN:Bo\nEND:VCARD\n"
)

class T(unittest.TestCase):
    def test_merge_by_email(self):
        cards = parse_text(SRC)
        merged = merge_cards(cards)
        self.assertEqual(len(merged), 2)
        al = [c for c in merged if c.get("EMAIL") == "a@x"][0]
        self.assertEqual(al.get("TEL"), "123")

    def test_names(self):
        self.assertEqual(names(parse_text(SRC)), ["Al", "Bo"])

