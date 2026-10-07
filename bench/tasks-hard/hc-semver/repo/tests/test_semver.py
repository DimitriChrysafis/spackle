import unittest
from semver import Version, sort, max_version

class TestParse(unittest.TestCase):
    def test_basic(self):
        v = Version("1.2.3")
        self.assertEqual((v.major, v.minor, v.patch), (1, 2, 3))
        self.assertIsNone(v.pre)

    def test_pre_and_build(self):
        v = Version("2.0.0-rc.1+sha.5114f85")
        self.assertEqual(v.pre, ["rc", "1"])
        self.assertEqual(v.build, "sha.5114f85")

    def test_invalid(self):
        for bad in ["1.2", "x.y.z", "1.0.0-", "", "1.0.0.4"]:
            with self.assertRaises(ValueError, msg=bad):
                Version(bad)

class TestOrder(unittest.TestCase):
    def test_core_order(self):
        self.assertLess(Version("1.2.3"), Version("1.2.4"))
        self.assertLess(Version("1.9.0"), Version("1.10.0"))
        self.assertLess(Version("0.9.9"), Version("1.0.0"))

    def test_prerelease_before_release(self):
        self.assertLess(Version("1.0.0-alpha"), Version("1.0.0"))

    def test_prerelease_ordering(self):
        chain = ["1.0.0-alpha", "1.0.0-alpha.1", "1.0.0-alpha.beta",
                 "1.0.0-beta", "1.0.0-beta.2", "1.0.0-beta.11",
                 "1.0.0-rc.1", "1.0.0"]
        for a, b in zip(chain, chain[1:]):
            self.assertLess(Version(a), Version(b), f"{a} < {b}")

    def test_equality(self):
        self.assertEqual(Version("1.0.0+build.1"), Version("1.0.0+other"))
        self.assertNotEqual(Version("1.0.0-alpha"), Version("1.0.0"))

class TestHelpers(unittest.TestCase):
    def test_sort(self):
        out = sort(["1.10.0", "1.9.0", "1.0.0-rc.1", "1.0.0"])
        self.assertEqual([str(v) for v in out],
                         ["1.0.0-rc.1", "1.0.0", "1.9.0", "1.10.0"])

    def test_max(self):
        self.assertEqual(str(max_version(["0.1.0", "0.2.0", "0.2.0-rc.1"])),
                         "0.2.0")

