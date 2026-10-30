# Роли: полиморфный вызов над 300000 фигурами четырёх видов.
class Rect
  def initialize(w, h); @w, @h = w, h; end
  def area; @w * @h; end
  def perimeter; 2 * (@w + @h); end
  def weight; 1; end
end

class Square
  def initialize(side); @side = side; end
  def area; @side * @side; end
  def perimeter; 4 * @side; end
  def weight; 2; end
end

class Triangle
  def initialize(a, b, c); @a, @b, @c = a, b, c; end
  def area; @a * @b % 1000; end
  def perimeter; @a + @b + @c; end
  def weight; 3; end
end

class Circle
  def initialize(r); @r = r; end
  def area; 314 * @r * @r % 100000; end
  def perimeter; 628 * @r % 10000; end
  def weight; 4; end
end

def shape(i)
  case i % 4
  when 0 then Rect.new(i % 17 + 1, i % 23 + 1)
  when 1 then Square.new(i % 31 + 1)
  when 2 then Triangle.new(i % 11 + 3, i % 13 + 4, i % 7 + 5)
  else Circle.new(i % 19 + 1)
  end
end

def score(shapes)
  shapes.reduce(0) { |total, s| total + s.area * s.weight + s.perimeter }
end

shapes = (0...300000).map { |i| shape(i) }
puts "#{shapes.size} #{(0...3).reduce(0) { |sum, _| sum + score(shapes) }}"
