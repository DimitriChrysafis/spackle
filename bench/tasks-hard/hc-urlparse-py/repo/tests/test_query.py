import unittest
from urlp.query import parse_query, build_query

class T(unittest.TestCase):
    def test_parse(self):
        self.assertEqual(parse_query("a=1&b=2"), {"a": "1", "b": "2"})

    def test_repeat(self):
        self.assertEqual(parse_query("a=1&a=2"), {"a": ["1", "2"]})

    def test_build(self):
        self.assertEqual(build_query([("q", "a b"), ("n", 1)]),
                         "q=a%20b&n=1")

