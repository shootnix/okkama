# Точные целые: факториал 12000 и 150000-е число Фибоначчи.
def factorial(n):
    product = 1
    for k in range(1, n + 1):
        product *= k
    return product


def fibonacci(n):
    a, b = 0, 1
    for _ in range(n):
        a, b = b, a + b
    return a


f = factorial(12000)
g = fibonacci(150000)
print(len(str(f)) + len(str(g)), f % 1000000007, g % 1000000007)
