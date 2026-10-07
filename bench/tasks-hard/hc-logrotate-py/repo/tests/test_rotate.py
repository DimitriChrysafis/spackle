import unittest
from datetime import datetime
from rotator import Period, next_rotation, plan_rotations

class T(unittest.TestCase):
    def test_daily(self):
        n = next_rotation(Period("daily"), datetime(2025, 3, 10, 15))
        self.assertEqual(n, datetime(2025, 3, 11))

    def test_weekly(self):
        # 2025-03-10 is a Monday; next weekly boundary after it is 17th
        n = next_rotation(Period("weekly"), datetime(2025, 3, 10))
        self.assertEqual(n, datetime(2025, 3, 17))
        n2 = next_rotation(Period("weekly"), datetime(2025, 3, 12))
        self.assertEqual(n2, datetime(2025, 3, 17))

    def test_monthly(self):
        n = next_rotation(Period("monthly"), datetime(2025, 1, 31))
        # next 1st-of-month after Jan 31 is Feb 1
        self.assertEqual(n, datetime(2025, 2, 1))

    def test_monthly_dec(self):
        n = next_rotation(Period("monthly"), datetime(2025, 12, 15))
        self.assertEqual(n, datetime(2026, 1, 1))

    def test_plan(self):
        plan = plan_rotations(Period("monthly"), datetime(2025, 11, 20), 3)
        self.assertEqual(plan, [datetime(2025, 12, 1),
                                datetime(2026, 1, 1),
                                datetime(2026, 2, 1)])
