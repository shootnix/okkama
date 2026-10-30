# Точные целые: факториал 12000 и 150000-е число Фибоначчи.
factorial = (1..12000).reduce(1) { |product, k| product * k }
a, b = 0, 1
150000.times { a, b = b, a + b }
puts "#{factorial.to_s.length + a.to_s.length} #{factorial % 1000000007} #{a % 1000000007}"
