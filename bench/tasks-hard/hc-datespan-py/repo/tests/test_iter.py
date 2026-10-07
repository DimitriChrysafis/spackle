import unittest
from datetime import date
from spans import DateRange
from spans.iterate import iter_days, chunks, week_start

class T(unittest.TestCase):
    def test_iter_days(self):
        r = DateRange("2025-01-01", "2025-01-04")
        self.assertEqual([d.isoformat() for d in iter_days(r)],
                         ["2025-01-01", "2025-01-02", "2025-01-03"])

    def test_chunks(self):
        r = DateRange("2025-01-01", "2025-01-08")
        parts = list(chunks(r, 3))
        self.assertEqual([p.days() for p in parts], [3, 3, 1])

    def test_week_start(self):
        self.assertEqual(week_start(date(2025, 3, 12)).isoformat(),
                         "2025-03-10")

