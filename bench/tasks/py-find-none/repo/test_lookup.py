import unittest
import lookup

class T(unittest.TestCase):
    def test_found(self):
        self.assertEqual(lookup.first_even([1, 4, 6]), 4)
    def test_none(self):
        self.assertIsNone(lookup.first_even([1, 3, 5]))
        self.assertIsNone(lookup.first_even([]))

unittest.main()
