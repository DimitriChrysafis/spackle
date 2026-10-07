import unittest
from calc import evaluate

class T(unittest.TestCase):
    def test_arith(self):
        self.assertEqual(evaluate("1 + 2 * 3"), 7)
        self.assertEqual(evaluate("(1 + 2) * 3"), 9)
        self.assertEqual(evaluate("10 / 4"), 2.5)
        self.assertEqual(evaluate("10 % 3"), 1)

    def test_pow_right_assoc(self):
        self.assertEqual(evaluate("2 ** 3 ** 2"), 512)

    def test_unary_precedence(self):
        # -2**2 is -(2**2) = -4, matching Python math convention
        self.assertEqual(evaluate("-2 ** 2"), -4)
        self.assertEqual(evaluate("2 ** -2"), 0.25)
        self.assertEqual(evaluate("-2 ** 3"), -8)

    def test_parens_unary(self):
        self.assertEqual(evaluate("(-2) ** 2"), 4)
        self.assertEqual(evaluate("-(2 + 3)"), -5)

    def test_div_zero(self):
        with self.assertRaises(ZeroDivisionError):
            evaluate("1 / 0")

    def test_syntax(self):
        with self.assertRaises(SyntaxError):
            evaluate("2 +")
