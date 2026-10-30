# Целочисленная арифметика в цикле: линейный конгруэнтный генератор.
def run(left, state, sum)
  while left > 0
    state = (state * 1103515245 + 12345) % 2147483648
    sum += state % 1000
    left -= 1
  end
  sum
end

puts run(3000000, 42, 0)
