import unittest
from tinytmpl.filters import apply_filter

class T(unittest.TestCase):
    def test_upper(self):
        self.assertEqual(apply_filter("hi", "upper"), "HI")

    def test_length(self):
        self.assertEqual(apply_filter([1, 2], "length"), 2)

    def test_unknown(self):
        with self.assertRaises(ValueError):
            apply_filter("x", "bogus")

