import unittest
import paths

class T(unittest.TestCase):
    def test_square(self):
        self.assertEqual(paths.paths(2, 2), 2)
    def test_bigger(self):
        self.assertEqual(paths.paths(3, 3), 6)
        self.assertEqual(paths.paths(4, 4), 20)

unittest.main()
