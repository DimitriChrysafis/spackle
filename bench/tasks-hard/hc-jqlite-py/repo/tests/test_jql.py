import unittest
from jql import run

DOC = {
    "user": {"name": "al", "tags": ["a", "b"]},
    "items": [{"id": 1}, {"id": 2}, {"id": 3}],
    "n": 5,
}

class T(unittest.TestCase):
    def test_identity(self):
        self.assertEqual(run(".", DOC), [DOC])

    def test_field(self):
        self.assertEqual(run(".n", DOC), [5])
        self.assertEqual(run(".user.name", DOC), ["al"])

    def test_index(self):
        self.assertEqual(run(".user.tags[0]", DOC), ["a"])
        self.assertEqual(run(".items[2].id", DOC), [3])

    def test_iterate(self):
        self.assertEqual(run(".items[].id", DOC), [1, 2, 3])
        self.assertEqual(run(".user.tags[]", DOC), ["a", "b"])

    def test_object_values(self):
        self.assertEqual(run(".kv[]", {"kv": {"x": 1, "y": 2}}), [1, 2])

    def test_pipe(self):
        self.assertEqual(run(".items | .[0]", DOC), [{"id": 1}])
        self.assertEqual(run(".user | .name", DOC), ["al"])

    def test_optional(self):
        self.assertEqual(run(".missing?", DOC), [])
        self.assertEqual(run(".missing.x?", DOC), [])

    def test_missing_raises(self):
        with self.assertRaises(KeyError):
            run(".missing", DOC)
