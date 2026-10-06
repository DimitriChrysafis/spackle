import unittest
import calc

class T(unittest.TestCase):
    def test_add(self):
        self.assertEqual(calc.add(2, 3), 5)
    def test_mul(self):
        self.assertEqual(calc.mul(2, 3), 6)

unittest.main()
