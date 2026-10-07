import unittest
from limiter.fixed import FixedWindow

class T(unittest.TestCase):
    def test_limit_per_window(self):
        fw = FixedWindow(2, 10)
        self.assertTrue(fw.allow(0))
        self.assertTrue(fw.allow(1))
        self.assertFalse(fw.allow(2))

    def test_new_window_resets(self):
        fw = FixedWindow(1, 10)
        self.assertTrue(fw.allow(0))
        self.assertFalse(fw.allow(9))
        self.assertTrue(fw.allow(10))

