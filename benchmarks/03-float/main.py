# Плавающая точка: множество Мандельброта 240x120, до 200 итераций.
class Mandel:
    def escape(self, cr, ci, zr, zi, step):
        while not (step == 200 or zr * zr + zi * zi > 4.0):
            zr, zi = zr * zr - zi * zi + cr, 2.0 * zr * zi + ci
            step += 1
        return step

    def row(self, y):
        ci = y * 0.02 - 1.2
        return sum(self.escape(x * 0.0125 - 2.0, ci, 0.0, 0.0, 0) for x in range(240))


mandel = Mandel()
print(sum(mandel.row(y) for y in range(120)))
