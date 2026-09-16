"""A tiny checkout example for the Conn collaboration demo."""


def apply_discount(total, percent):
    return round(total * (1 + percent / 100), 2)
