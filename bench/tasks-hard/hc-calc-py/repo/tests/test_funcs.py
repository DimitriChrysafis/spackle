import unittest
from calc.functions import call, FUNCS

class T(unittest.TestCase):
    def test_funcs(self):
        self.assertEqual(call("sqrt", [9]), 3)
        self.assertEqual(call("abs", [-2]), 2)
        self.assertEqual(call("min", [3, 1]), 1)
        self.assertEqual(call("max", [3, 1]), 3)

    def test_unknown(self):
        with self.assertRaises(ValueError):
            call("nope", [])

    def test_registry(self):
        self.assertIn("floor", FUNCS)

