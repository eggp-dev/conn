import unittest
from price import apply_discount


class CheckoutTests(unittest.TestCase):
    def test_twenty_percent_off(self):
        self.assertEqual(apply_discount(120, 20), 96)

    def test_no_discount(self):
        self.assertEqual(apply_discount(120, 0), 120)


if __name__ == '__main__':
    unittest.main()
