import unittest
import words

class T(unittest.TestCase):
    def test_simple(self):
        self.assertTrue(words.is_palindrome("racecar"))
    def test_case_space(self):
        self.assertTrue(words.is_palindrome("Race car"))
        self.assertTrue(words.is_palindrome("A man a plan a canal Panama"))
    def test_negative(self):
        self.assertFalse(words.is_palindrome("hello"))

unittest.main()
