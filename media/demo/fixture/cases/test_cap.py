import unittest
from price import apply_discount

class DiscountLimitTests(unittest.TestCase):
    def test_discount_cannot_make_price_negative(self):
        self.assertEqual(apply_discount(120, 125), 0)
