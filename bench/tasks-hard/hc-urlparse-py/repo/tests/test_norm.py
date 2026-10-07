import unittest
from urlp.normalize import normalize

class T(unittest.TestCase):
    def test_scheme_case(self):
        self.assertEqual(str(normalize("HTTP://EX.com/A")),
                         "http://ex.com/A")

    def test_default_port_dropped(self):
        self.assertEqual(str(normalize("http://ex.com:80/")),
                         "http://ex.com/")

    def test_dot_segments(self):
        self.assertEqual(str(normalize("http://x/a/../b/./c")),
                         "http://x/b/c")

