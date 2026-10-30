# Замыкания и функции высшего порядка: композиция, захват, лямбды в массивах.
def compose(f, g)
  ->(value) { g.(f.(value)) }
end

def adder(amount)
  ->(value) { value + amount }
end

def multiplier(factor)
  ->(value) { value * factor % 1000003 }
end

stages = (0...8).map { |i| i.even? ? adder(i + 1) : multiplier(i + 2) }
combined = stages.reduce(->(value) { value }) { |composed, stage| compose(composed, stage) }
total = (0...600000).reduce(0) { |sum, i| (sum + combined.(i)) % 1000000007 }
counters = (0...50000).map { |i| adder(i) }
applied = counters.reduce(0) { |sum, add| add.(sum) % 1000003 }
puts "#{total} #{applied}"
