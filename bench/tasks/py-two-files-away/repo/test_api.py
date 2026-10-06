import unittest
import api

class T(unittest.TestCase):
    def test_get_user(self):
        u = api.get_user(2)
        self.assertEqual(u["id"], 2)
        self.assertEqual(u["name"], "grace")
        self.assertEqual(u["email"], "grace@x.io")

unittest.main()
