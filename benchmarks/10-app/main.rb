# Приложение: модель, каталог, правила цены, ошибки домена, котировки и отчёт;
# неизменяемые значения, как в main.ok. В Ruby нет async: котировка — вызов.

class Money
  attr_reader :cents

  def initialize(cents)
    @cents = cents
  end

  def plus(other)
    Money.new(@cents + other.cents)
  end

  def minus(other)
    Money.new(@cents - other.cents)
  end

  def times(amount)
    Money.new(@cents * amount)
  end

  def percent(rate)
    scaled = @cents * rate
    Money.new((scaled - scaled % 100) / 100)
  end

  def to_s
    fraction = @cents % 100
    whole = (@cents - fraction) / 100
    "#{whole}.#{fraction < 10 ? '0' : ''}#{fraction}"
  end
end

class Product
  attr_reader :id, :name, :category, :stock

  def initialize(id:, name:, category:, price:, stock:)
    @id, @name, @category, @price, @stock = id, name, category, price, stock
  end

  def price
    Money.new(@price)
  end
end

class Customer
  attr_reader :id, :name, :tier

  def initialize(id:, name:, tier:)
    @id, @name, @tier = id, name, tier
  end
end

class Line
  attr_reader :product, :quantity

  def initialize(product:, quantity:)
    @product, @quantity = product, quantity
  end

  def subtotal
    @product.price.times(@quantity)
  end
end

class Order
  attr_reader :id, :customer, :lines

  def initialize(id:, customer:, lines: [])
    @id, @customer, @lines = id, customer, lines
  end

  def items
    @lines.reduce(0) { |count, line| count + line.quantity }
  end

  def subtotal
    @lines.reduce(Money.new(0)) { |sum, line| sum.plus(line.subtotal) }
  end

  def added(line)
    Order.new(id: @id, customer: @customer, lines: @lines + [line])
  end
end

class OutOfStock < StandardError
  attr_reader :product, :available

  def initialize(product:, available:)
    super("product #{product} is out of stock")
    @product, @available = product, available
  end
end

class Catalog
  attr_reader :size

  def initialize(size:)
    @size = size
  end

  def product(id)
    Product.new(id: id, name: "product-#{id}", category: id * 7 % 10, price: id * 389 % 20000 + 199, stock: id * 13 % 40 + 1)
  end

  def customer(id)
    Customer.new(id: id, name: "customer-#{id}", tier: id % 4)
  end

  def reserved(product, quantity)
    raise OutOfStock.new(product: product.id, available: product.stock) if quantity > product.stock
    quantity
  end
end

class BulkDiscount
  def name; "bulk"; end

  def adjusted(order, total)
    total.cents > 200000 ? total.minus(total.percent(5)) : total
  end
end

class LoyaltyDiscount
  def name; "loyalty"; end

  def adjusted(order, total)
    total.minus(total.percent(order.customer.tier * 2))
  end
end

class CategoryPromotion
  def initialize(category:)
    @category = category
  end

  def name; "promotion"; end

  def adjusted(order, total)
    promoted = order.lines.select { |line| line.product.category == @category }
    discount = promoted.reduce(Money.new(0)) { |sum, line| sum.plus(line.subtotal.percent(10)) }
    total.minus(discount)
  end
end

class Tax
  def name; "tax"; end

  def adjusted(order, total)
    total.plus(total.percent(20))
  end
end

class PriceEngine
  def initialize(rules:)
    @rules = rules
  end

  def total(order)
    @rules.reduce(order.subtotal) { |sum, rule| rule.adjusted(order, sum) }
  end
end

class OrderSource
  def initialize(catalog:)
    @catalog = catalog
  end

  def order(id)
    order = Order.new(id: id, customer: @catalog.customer(id * 31 % 5000))
    (id % 8 + 1).times do |index|
      product = @catalog.product((id * 97 + index * 1031) % @catalog.size)
      wanted = (id + index * 3) % 12 + 1
      quantity = begin
        @catalog.reserved(product, wanted)
      rescue OutOfStock => shortage
        shortage.available
      end
      order = order.added(Line.new(product: product, quantity: quantity))
    end
    order
  end
end

class Report
  NAMES = %w[tools garden books music toys food sports office kitchen auto].freeze

  def initialize(revenue: Array.new(10, 0), orders: 0, items: 0, cents: 0)
    @revenue, @orders, @items, @cents = revenue, orders, items, cents
  end

  def recorded(order, total)
    lines = order.lines
    revenue = (0...10).map do |category|
      own = lines.select { |line| line.product.category == category }
      @revenue[category] + own.reduce(0) { |sum, line| sum + line.subtotal.cents }
    end
    Report.new(revenue: revenue, orders: @orders + 1, items: @items + order.items, cents: @cents + total.cents)
  end

  def text
    rows = (0...10).map { |category| "#{NAMES[category]}: #{Money.new(@revenue[category])}" }
    average = Money.new((@cents - @cents % @orders) / @orders)
    summary = ["orders: #{@orders}", "items: #{@items}", "revenue: #{Money.new(@cents)}", "average: #{average}"]
    (rows + summary).join("\n")
  end
end

class Quotes
  def initialize(engine:)
    @engine = engine
  end

  def quote(order)
    @engine.total(order)
  end
end

rules = [BulkDiscount.new, CategoryPromotion.new(category: 3), LoyaltyDiscount.new, Tax.new]
quotes = Quotes.new(engine: PriceEngine.new(rules: rules))
source = OrderSource.new(catalog: Catalog.new(size: 2000))
report = Report.new
30000.times do |id|
  order = source.order(id)
  total = quotes.quote(order)
  report = report.recorded(order, total)
end
text = report.text
puts text
puts "length: #{text.length}"
