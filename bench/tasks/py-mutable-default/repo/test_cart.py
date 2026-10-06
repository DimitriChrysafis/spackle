import unittest
import cart

class T(unittest.TestCase):
    def test_isolated(self):
        self.assertEqual(cart.add_item("a"), ["a"])
        self.assertEqual(cart.add_item("b"), ["b"])

unittest.main()
