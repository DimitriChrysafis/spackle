import unittest
import dedupe

class T(unittest.TestCase):
    def test_order(self):
        self.assertEqual(dedupe.dedupe([3, 1, 3, 2]), [3, 1, 2])
        self.assertEqual(dedupe.dedupe([9, 9, 8, 9]), [9, 8])

unittest.main()
