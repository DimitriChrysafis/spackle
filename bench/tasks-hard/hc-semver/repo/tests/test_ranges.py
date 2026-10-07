import unittest
from rangecmp import satisfies, filter_satisfying
from semver import Version

class T(unittest.TestCase):
    def test_caret(self):
        self.assertTrue(satisfies("1.4.0", "^1.2.0"))
        self.assertFalse(satisfies("2.0.0", "^1.2.0"))
        self.assertFalse(satisfies("0.4.0", "^0.2.0"))

    def test_tilde(self):
        self.assertTrue(satisfies("1.2.9", "~1.2.0"))
        self.assertFalse(satisfies("1.3.0", "~1.2.0"))

    def test_bounds(self):
        self.assertTrue(satisfies("1.5.0", ">=1.0.0 <2.0.0"))
        self.assertFalse(satisfies("2.0.0", ">=1.0.0 <2.0.0"))

    def test_exact(self):
        self.assertTrue(satisfies("1.2.3", "1.2.3"))
        self.assertFalse(satisfies("1.2.4", "1.2.3"))

    def test_filter(self):
        vs = [Version(x) for x in ["1.0.0", "1.5.0", "2.0.0"]]
        self.assertEqual(filter_satisfying(vs, "^1.0.0"), ["1.0.0", "1.5.0"])

