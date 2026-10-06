import unittest
import text

class T(unittest.TestCase):
    def test_reverse_words(self):
        self.assertEqual(text.reverse_words("hello world foo"), "foo world hello")
        self.assertEqual(text.reverse_words("one"), "one")

unittest.main()
