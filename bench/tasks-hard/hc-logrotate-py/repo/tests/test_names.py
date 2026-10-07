import os
import tempfile
import unittest
from datetime import datetime
from rotator.names import dated_name, is_dated, next_seq

class T(unittest.TestCase):
    def test_dated(self):
        n = dated_name("app.log", datetime(2025, 3, 4))
        self.assertEqual(n, "app.log.20250304")
        self.assertTrue(is_dated(n))
        self.assertFalse(is_dated("app.log"))

    def test_next_seq(self):
        d = tempfile.mkdtemp()
        self.assertEqual(next_seq(d, "x"), "x.1")
        open(os.path.join(d, "x.1"), "w").close()
        open(os.path.join(d, "x.2"), "w").close()
        self.assertEqual(next_seq(d, "x"), "x.3")

