import unittest
import money

class T(unittest.TestCase):
    def test_cents(self):
        self.assertEqual(money.total(["0.10", "0.20", "0.30"]), 0.6)
    def test_no_drift(self):
        self.assertEqual(money.total(["0.01"] * 1000), 10.0)

unittest.main()
