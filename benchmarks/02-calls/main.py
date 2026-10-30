# Вызовы: рекурсивный fib (~7 млн вызовов).
class Fib:
    def fib(self, n):
        return n if n < 2 else self.fib(n - 1) + self.fib(n - 2)


print(Fib().fib(32))
