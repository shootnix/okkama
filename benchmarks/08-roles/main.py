# Роли: полиморфный вызов над 300000 фигурами четырёх видов.
class Rect:
    __slots__ = ("w", "h")
    def __init__(self, w, h): self.w, self.h = w, h
    def area(self): return self.w * self.h
    def perimeter(self): return 2 * (self.w + self.h)
    def weight(self): return 1


class Square:
    __slots__ = ("side",)
    def __init__(self, side): self.side = side
    def area(self): return self.side * self.side
    def perimeter(self): return 4 * self.side
    def weight(self): return 2


class Triangle:
    __slots__ = ("a", "b", "c")
    def __init__(self, a, b, c): self.a, self.b, self.c = a, b, c
    def area(self): return self.a * self.b % 1000
    def perimeter(self): return self.a + self.b + self.c
    def weight(self): return 3


class Circle:
    __slots__ = ("r",)
    def __init__(self, r): self.r = r
    def area(self): return 314 * self.r * self.r % 100000
    def perimeter(self): return 628 * self.r % 10000
    def weight(self): return 4


def shape(i):
    kind = i % 4
    if kind == 0:
        return Rect(i % 17 + 1, i % 23 + 1)
    if kind == 1:
        return Square(i % 31 + 1)
    if kind == 2:
        return Triangle(i % 11 + 3, i % 13 + 4, i % 7 + 5)
    return Circle(i % 19 + 1)


def score(shapes):
    return sum(s.area() * s.weight() + s.perimeter() for s in shapes)


shapes = [shape(i) for i in range(300000)]
print(len(shapes), sum(score(shapes) for _ in range(3)))
