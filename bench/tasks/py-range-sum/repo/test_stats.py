import unittest
import stats

class T(unittest.TestCase):
    def test_total(self):
        self.assertEqual(stats.total(10), 55)
        self.assertEqual(stats.total(0), 0)
        self.assertEqual(stats.total(1), 1)

unittest.main()
