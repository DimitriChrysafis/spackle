import unittest
from limiter import RateLimiter, TokenBucket

class TestWindow(unittest.TestCase):
    def test_burst(self):
        rl = RateLimiter(3, 10.0)
        self.assertTrue(rl.allow(0.0))
        self.assertTrue(rl.allow(0.5))
        self.assertTrue(rl.allow(1.0))
        self.assertFalse(rl.allow(1.5))
        self.assertFalse(rl.allow(2.0))

    def test_expire(self):
        rl = RateLimiter(2, 10.0)
        rl.allow(0.0)
        rl.allow(1.0)
        self.assertFalse(rl.allow(2.0))
        self.assertTrue(rl.allow(11.5))

    def test_boundary(self):
        rl = RateLimiter(2, 10.0)
        rl.allow(100.0)
        rl.allow(105.0)
        self.assertFalse(rl.allow(109.0))
        # at 111.0 the call at 100 is > 10s old and frees a slot
        self.assertTrue(rl.allow(111.0))

    def test_remaining(self):
        rl = RateLimiter(4, 60.0)
        rl.allow(0)
        self.assertEqual(rl.remaining(0), 3)

    def test_next_free(self):
        rl = RateLimiter(1, 5.0)
        rl.allow(10.0)
        self.assertFalse(rl.allow(11.0))
        self.assertAlmostEqual(rl.next_free(11.0), 4.0)
        self.assertAlmostEqual(rl.next_free(16.0), 0.0)

class TestBucket(unittest.TestCase):
    def test_capacity(self):
        b = TokenBucket(5, 1.0)
        for _ in range(5):
            self.assertTrue(b.take(0.0))
        self.assertFalse(b.take(0.0))

    def test_refill(self):
        b = TokenBucket(2, 1.0)
        b.take(0.0)
        b.take(0.0)
        self.assertFalse(b.take(0.0))
        self.assertTrue(b.take(1.5))

