import unittest
from envcfg.interpolate import expand, expand_all

class T(unittest.TestCase):
    def test_expand(self):
        self.assertEqual(expand("x ${A} y", {"A": "1"}), "x 1 y")

    def test_default(self):
        self.assertEqual(expand("${B:-d}"), "d")
        self.assertEqual(expand("${B:-d}", {"B": "v"}), "v")

    def test_unknown_left_alone(self):
        self.assertEqual(expand("${NOPE_12345}"), "${NOPE_12345}")

    def test_expand_all(self):
        out = expand_all({"a": "${X:-9}", "b": "z"}, {"X": "3"})
        self.assertEqual(out, {"a": "3", "b": "z"})

