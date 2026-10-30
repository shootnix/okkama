# Объекты: миллион шагов неизменяемых объектов (каждый шаг — новый объект).
class Particle
  attr_reader :bounces

  def initialize(x, y, vx, vy, bounces = 0)
    @x, @y, @vx, @vy, @bounces = x, y, vx, vy, bounces
  end

  def energy
    @vx * @vx + @vy * @vy
  end

  def position
    @x * 1000 + @y
  end

  def moved
    nx = @x + @vx
    ny = @y + @vy
    hit_x = nx < 0 || nx > 1000
    hit_y = ny < 0 || ny > 1000
    Particle.new(hit_x ? @x : nx, hit_y ? @y : ny, hit_x ? -@vx : @vx, hit_y ? -@vy : @vy,
                 hit_x || hit_y ? @bounces + 1 : @bounces)
  end
end

def simulate(id, steps)
  particle = Particle.new(id * 37 % 1000, id * 91 % 1000, id % 13 + 1, id % 7 + 2)
  steps.times { particle = particle.moved }
  particle.position + particle.bounces * 7 + particle.energy
end

puts (0...400).reduce(0) { |sum, id| sum + simulate(id, 2500) }
