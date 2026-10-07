import unittest
from urlp import parse, join

class T(unittest.TestCase):
    def test_full(self):
        u = parse("https://u:p@ex.com:8443/a/b?x=1#frag")
        self.assertEqual(u.scheme, "https")
        self.assertEqual(u.userinfo, "u:p")
        self.assertEqual(u.host, "ex.com")
        self.assertEqual(u.port, 8443)
        self.assertEqual(u.path, "/a/b")
        self.assertEqual(u.query, "x=1")
        self.assertEqual(u.fragment, "frag")

    def test_no_port(self):
        u = parse("http://example.com/")
        self.assertIsNone(u.port)
        self.assertEqual(u.host, "example.com")

    def test_relative_no_scheme(self):
        # 'host:port/path' has NO scheme - it is authority-only
        u = parse("example.com:8080/p")
        self.assertIsNone(u.scheme)
        self.assertEqual(u.host, "example.com")
        self.assertEqual(u.port, 8080)
        self.assertEqual(u.path, "/p")

    def test_str_roundtrip(self):
        s = "https://ex.com:9/a?b#c"
        self.assertEqual(str(parse(s)), s)

    def test_join(self):
        self.assertEqual(str(join("https://a.com/x/y", "z")),
                         "https://a.com/x/z")
        self.assertEqual(str(join("https://a.com/x/y", "/root")),
                         "https://a.com/root")
        self.assertEqual(str(join("https://a.com/x/", "//cdn.io/f")),
                         "https://cdn.io/f")
