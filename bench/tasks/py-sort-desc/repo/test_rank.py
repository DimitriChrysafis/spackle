import unittest
import rank

class T(unittest.TestCase):
    def test_top(self):
        self.assertEqual(rank.top_scores([3, 1, 4], 2), [4, 3])
    def test_more_than_len(self):
        self.assertEqual(rank.top_scores([1], 5), [1])

unittest.main()
