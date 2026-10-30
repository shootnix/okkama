# Объекты: миллион шагов неизменяемых объектов (каждый шаг — новый объект).
class Particle:
    __slots__ = ("x", "y", "vx", "vy", "bounces")

    def __init__(self, x, y, vx, vy, bounces=0):
        self.x, self.y, self.vx, self.vy, self.bounces = x, y, vx, vy, bounces

    def energy(self):
        return self.vx * self.vx + self.vy * self.vy

    def position(self):
        return self.x * 1000 + self.y

    def moved(self):
        nx = self.x + self.vx
        ny = self.y + self.vy
        hit_x = nx < 0 or nx > 1000
        hit_y = ny < 0 or ny > 1000
        return Particle(
            self.x if hit_x else nx,
            self.y if hit_y else ny,
            -self.vx if hit_x else self.vx,
            -self.vy if hit_y else self.vy,
            self.bounces + 1 if hit_x or hit_y else self.bounces,
        )


def simulate(i, steps):
    particle = Particle(i * 37 % 1000, i * 91 % 1000, i % 13 + 1, i % 7 + 2)
    for _ in range(steps):
        particle = particle.moved()
    return particle.position() + particle.bounces * 7 + particle.energy()


print(sum(simulate(i, 2500) for i in range(400)))
