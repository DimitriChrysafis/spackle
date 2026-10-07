import unittest
from limiter import RateLimiter
from limiter.multi import Chain, AnyOf

class T(unittest.TestCase):
    def test_chain(self):
        fast = RateLimiter(1, 10.0)
        slow = RateLimiter(100, 10.0)
        c = Chain(fast, slow)
        self.assertTrue(c.allow(0.0))
        self.assertFalse(c.allow(1.0))

    def test_any(self):
        a = RateLimiter(0, 10.0) if False else RateLimiter(1, 10.0)
        b = RateLimiter(1, 10.0)
        c = AnyOf(a, b)
        self.assertTrue(c.allow(0.0))

