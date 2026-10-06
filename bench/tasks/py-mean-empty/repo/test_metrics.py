import unittest
import metrics

class T(unittest.TestCase):
    def test_mean(self):
        self.assertEqual(metrics.mean([2, 4]), 3.0)
    def test_empty(self):
        self.assertEqual(metrics.mean([]), 0.0)

unittest.main()
