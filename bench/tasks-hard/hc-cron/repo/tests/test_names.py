import unittest
from cronfield.names import parse_named, MONTHS, DAYS

class T(unittest.TestCase):
    def test_month_names(self):
        self.assertEqual(parse_named("jan-mar", 1, 12, MONTHS), [1, 2, 3])

    def test_day_names(self):
        self.assertEqual(parse_named("mon-fri", 0, 6, DAYS),
                         [1, 2, 3, 4, 5])

    def test_mixed(self):
        self.assertEqual(parse_named("feb,4", 1, 12, MONTHS), [2, 4])

