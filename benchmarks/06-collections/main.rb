# Коллекции: map/select/reduce, сортировка, хэши и частоты.
values = (0...200000).map { |i| (i * 7919 + 13) % 10007 }
evens = values.select { |n| n % 2 == 0 }
squares = evens.map { |n| n * n % 1000003 }
sum = squares.reduce(0) { |total, n| total + n }
sorted = values[0, 50000].sort
buckets = Hash.new(0)
values[0, 4000].each { |n| buckets[n % 97] += 1 }
index = { "low" => values.count { |n| n < 5000 }, "high" => values.count { |n| n >= 5000 } }
puts [sum, sorted[0], sorted[49999], buckets.size, buckets[0], index["low"], index["high"]].join(" ")
