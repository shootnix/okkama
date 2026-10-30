# Строки: сборка CSV на 30000 строк, split, замены, длины и разбор чисел.
def line(id)
  "id=#{id};name=item-#{id * 7 % 1000};price=#{id * 13 % 9973}"
end

def price(text)
  Integer(text.split(";")[2].gsub("price=", ""), 10)
end

text = (0...30000).map { |id| line(id) }.join("\n")
lines = text.split("\n")
total = lines.reduce(0) { |sum, row| sum + price(row) }
renamed = text.gsub("item-", "product_")
puts "#{lines.size} #{total} #{renamed.length} #{text.length}"
