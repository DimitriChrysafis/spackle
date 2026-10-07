import unittest
from envcfg import load
from envcfg.schema import validate

class T(unittest.TestCase):
    def test_valid(self):
        c = load(defaults={"port": "80", "debug": "on"})
        out = validate(c, {"port": "int", "debug": "bool"})
        self.assertEqual(out, {"port": 80, "debug": True})

    def test_missing(self):
        c = load(defaults={})
        with self.assertRaises(KeyError):
            validate(c, {"x": "int"})

    def test_bad_type(self):
        c = load(defaults={"n": "abc"})
        with self.assertRaises(ValueError):
            validate(c, {"n": "int"})

