from src.utils import add_numbers, multiply_numbers


def test_add():
    assert add_numbers(1, 2) == 3


def test_multiply():
    assert multiply_numbers(3, 4) == 12
