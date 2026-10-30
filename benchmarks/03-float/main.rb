# Плавающая точка: множество Мандельброта 240x120, до 200 итераций.
class Mandel
  def escape(cr, ci, zr, zi, step)
    until step == 200 || zr * zr + zi * zi > 4.0
      zr, zi = zr * zr - zi * zi + cr, 2.0 * zr * zi + ci
      step += 1
    end
    step
  end

  def row(y)
    ci = y * 0.02 - 1.2
    (0...240).reduce(0) { |total, x| total + escape(x * 0.0125 - 2.0, ci, 0.0, 0.0, 0) }
  end
end

mandel = Mandel.new
puts (0...120).reduce(0) { |sum, y| sum + mandel.row(y) }
