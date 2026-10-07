import unittest
from vcard import parse_text, write

SAMPLE = (
    "BEGIN:VCARD\r\n"
    "VERSION:3.0\r\n"
    "FN:Ada Lovelace\r\n"
    "N:Lovelace;Ada;;;\r\n"
    "EMAIL;TYPE=HOME:ada@example.com\r\n"
    "END:VCARD\r\n"
)

class TestParse(unittest.TestCase):
    def test_basic(self):
        cards = parse_text(SAMPLE)
        self.assertEqual(len(cards), 1)
        self.assertEqual(cards[0].get("FN"), "Ada Lovelace")
        self.assertEqual(cards[0].get("N"), "Lovelace;Ada;;;")

    def test_params(self):
        cards = parse_text(SAMPLE)
        emails = [p for p in cards[0].props if p.name == "EMAIL"]
        self.assertEqual(emails[0].params["TYPE"], "HOME")

    def test_folded_line(self):
        src = (
            "BEGIN:VCARD\r\n"
            "NOTE:This is a long note that was fo\r\n"
            " lded onto a second physical line.\r\n"
            "END:VCARD\r\n"
        )
        cards = parse_text(src)
        self.assertEqual(
            cards[0].get("NOTE"),
            "This is a long note that was folded onto a second physical line.",
        )

    def test_multi_card(self):
        src = SAMPLE + SAMPLE
        self.assertEqual(len(parse_text(src)), 2)

class TestWrite(unittest.TestCase):
    def test_roundtrip(self):
        cards = parse_text(SAMPLE)
        text = write(cards)
        again = parse_text(text)
        self.assertEqual(again[0].get("FN"), "Ada Lovelace")

    def test_long_prop_folds(self):
        cards = parse_text(SAMPLE)
        cards[0].props[0].value = "x" * 100
        for line in write(cards).splitlines():
            self.assertLessEqual(len(line), 75)
