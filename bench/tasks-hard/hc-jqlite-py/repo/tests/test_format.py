import unittest
from jql.format import to_json_lines, to_raw, to_rows

class T(unittest.TestCase):
    def test_json_lines(self):
        self.assertEqual(to_json_lines([{"a": 1}, 2]),
                         '{"a": 1}\n2')

    def test_raw(self):
        self.assertEqual(to_raw(["x", 1]), "x\n1")

    def test_rows(self):
        out = to_rows([{"a": 1, "b": 2}, {"a": 3}])
        self.assertEqual(out.split("\n")[0], "a,b")
        self.assertEqual(len(out.split("\n")), 3)

