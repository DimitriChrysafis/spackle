import unittest
from cronfield.schedule import parse_expr
from cronfield.next import next_minute, fires_today

class T(unittest.TestCase):
    def test_next_minute(self):
        s = parse_expr("*/20 * * * *")
        self.assertEqual(next_minute(s, 5), 20)
        self.assertIsNone(next_minute(s, 45))

    def test_fires(self):
        s = parse_expr("30 9 * * *")
        self.assertTrue(fires_today(s, 9, 30))
        self.assertFalse(fires_today(s, 9, 31))

