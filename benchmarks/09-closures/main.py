# Замыкания и функции высшего порядка: композиция, захват, функции в списках.
from functools import reduce


def compose(f, g):
    return lambda value: g(f(value))


def adder(amount):
    return lambda value: value + amount


def multiplier(factor):
    return lambda value: value * factor % 1000003


stages = [adder(i + 1) if i % 2 == 0 else multiplier(i + 2) for i in range(8)]
combined = reduce(compose, stages, lambda value: value)
total = reduce(lambda acc, i: (acc + combined(i)) % 1000000007, range(600000), 0)
counters = [adder(i) for i in range(50000)]
applied = reduce(lambda acc, add: add(acc) % 1000003, counters, 0)
print(total, applied)
