import unittest
from cronfield import parse
from cronfield.schedule import parse_expr, matches

class TestField(unittest.TestCase):
    def test_star(self):
        self.assertEqual(parse("*", 0, 59), list(range(60)))

    def test_single_and_list(self):
        self.assertEqual(parse("5", 0, 59), [5])
        self.assertEqual(parse("1,3,5", 0, 59), [1, 3, 5])

    def test_range(self):
        self.assertEqual(parse("9-12", 0, 23), [9, 10, 11, 12])

    def test_step_over_star(self):
        self.assertEqual(parse("*/15", 0, 59), [0, 15, 30, 45])

    def test_step_over_range(self):
        self.assertEqual(parse("1-10/3", 0, 59), [1, 4, 7, 10])
        self.assertEqual(parse("2-20/6", 0, 59), [2, 8, 14, 20])

    def test_step_from_single(self):
        # "5/20" means start at 5 and step to the top of the field
        self.assertEqual(parse("5/20", 0, 59), [5, 25, 45])

    def test_bounds(self):
        with self.assertRaises(ValueError):
            parse("0-99", 0, 59)
        with self.assertRaises(ValueError):
            parse("9-3", 0, 59)
        with self.assertRaises(ValueError):
            parse("*/0", 0, 59)

class TestExpr(unittest.TestCase):
    def test_parse_expr(self):
        sched = parse_expr("*/15 9-17 * * 1-5")
        self.assertEqual(sched["minute"], [0, 15, 30, 45])
        self.assertEqual(sched["dow"], [1, 2, 3, 4, 5])

    def test_matches(self):
        self.assertTrue(matches("0 9 * * *", 0, 9, 1, 1, 1))
        self.assertFalse(matches("0 9 * * *", 1, 9, 1, 1, 1))
        self.assertTrue(matches("*/30 * * * 1-5", 30, 12, 2, 6, 3))

