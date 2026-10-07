import os
import tempfile
import unittest
from datetime import datetime
from rotator.files import scan_dir, expired, latest

class T(unittest.TestCase):
    def setUp(self):
        self.d = tempfile.mkdtemp()
        for n in ["app.log.20250101", "app.log.20250201", "app.log"]:
            open(os.path.join(self.d, n), "w").close()

    def test_scan_skips_current(self):
        files = scan_dir(self.d)
        self.assertEqual(len(files), 2)

    def test_sorted(self):
        files = scan_dir(self.d)
        self.assertTrue(files[0][0] < files[1][0])
        self.assertTrue(latest(files).endswith("20250201"))

    def test_expired(self):
        files = scan_dir(self.d)
        out = expired(files, datetime(2025, 1, 15))
        self.assertEqual(len(out), 1)

