import unittest
import vowels

class T(unittest.TestCase):
    def test_count(self):
        self.assertEqual(vowels.count_vowels("hello"), 2)
        self.assertEqual(vowels.count_vowels("xyz"), 0)

unittest.main()
