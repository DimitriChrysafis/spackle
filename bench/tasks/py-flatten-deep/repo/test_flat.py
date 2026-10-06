import unittest
import flat

class T(unittest.TestCase):
    def test_shallow(self):
        self.assertEqual(flat.flatten([[1, 2], [3]]), [1, 2, 3])
    def test_deep(self):
        self.assertEqual(flat.flatten([[[1]]]), [1])
        self.assertEqual(flat.flatten([1, [2, [3, [4]]]]), [1, 2, 3, 4])

unittest.main()
