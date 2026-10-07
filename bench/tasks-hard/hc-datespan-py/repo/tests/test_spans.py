import unittest
from spans import Span, overlaps, merge, intersect, clamp, gap_days

class TestOverlap(unittest.TestCase):
    def test_overlap(self):
        self.assertTrue(overlaps(Span(0, 5), Span(4, 9)))
        self.assertTrue(overlaps(Span(0, 5), Span(0, 1)))

    def test_adjacent_not_overlapping(self):
        # [0,5) and [5,9) touch but do not overlap
        self.assertFalse(overlaps(Span(0, 5), Span(5, 9)))

    def test_disjoint(self):
        self.assertFalse(overlaps(Span(0, 2), Span(5, 9)))

    def test_containment(self):
        self.assertIn(3, Span(0, 5))
        self.assertNotIn(5, Span(0, 5))

class TestMerge(unittest.TestCase):
    def test_merge_overlapping(self):
        out = merge([Span(0, 5), Span(3, 9), Span(20, 25)])
        self.assertEqual(out, [Span(0, 9), Span(20, 25)])

    def test_adjacent_not_merged(self):
        out = merge([Span(0, 5), Span(5, 9)])
        self.assertEqual(out, [Span(0, 5), Span(5, 9)])

    def test_unsorted_input(self):
        out = merge([Span(8, 9), Span(0, 5), Span(4, 8)])
        self.assertEqual(out, [Span(0, 9)])

class TestIntersectClamp(unittest.TestCase):
    def test_intersect(self):
        self.assertEqual(intersect(Span(0, 5), Span(3, 9)), Span(3, 5))
        self.assertIsNone(intersect(Span(0, 1), Span(5, 9)))

    def test_clamp(self):
        self.assertEqual(clamp(Span(0, 10), 3, 7), Span(3, 7))

class TestDateRange(unittest.TestCase):
    def test_days(self):
        from spans import DateRange
        r = DateRange("2025-01-01", "2025-01-08")
        self.assertEqual(r.days(), 7)
        self.assertIn("2025-01-03", r)
        self.assertNotIn("2025-01-08", r)

    def test_gap_days(self):
        from spans import DateRange
        a = DateRange("2025-03-01", "2025-03-05")
        b = DateRange("2025-03-10", "2025-03-12")
        self.assertEqual(gap_days([a, b]), 5)
