import unittest
from jql.builtins import length, keys, has, first, last

class T(unittest.TestCase):
    def test_length(self):
        self.assertEqual(length([1, 2]), 2)
        self.assertEqual(length({"a": 1}), 1)

    def test_keys(self):
        self.assertEqual(keys({"b": 1, "a": 2}), ["a", "b"])

    def test_has(self):
        self.assertTrue(has({"x": 1}, "x"))

    def test_ends(self):
        self.assertEqual(first([3, 1]), 3)
        self.assertEqual(last([3, 1]), 1)

