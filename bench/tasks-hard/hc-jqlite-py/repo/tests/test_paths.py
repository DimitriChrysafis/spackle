import unittest
from jql.paths import leaf_paths, count_leaves

class T(unittest.TestCase):
    def test_paths(self):
        doc = {"a": {"b": 1}, "c": [5]}
        self.assertEqual(list(leaf_paths(doc)), [".a.b", ".c[0]"])

    def test_count(self):
        self.assertEqual(count_leaves({"x": {"y": {"z": 1}}}), 1)

