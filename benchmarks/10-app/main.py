# Приложение: модель, каталог, правила цены, ошибки домена, async-котировки и
# отчёт; неизменяемые значения, как в main.ok.
import asyncio


class Money:
    __slots__ = ("cents",)

    def __init__(self, cents):
        self.cents = cents

    def plus(self, other):
        return Money(self.cents + other.cents)

    def minus(self, other):
        return Money(self.cents - other.cents)

    def times(self, amount):
        return Money(self.cents * amount)

    def percent(self, rate):
        scaled = self.cents * rate
        return Money((scaled - scaled % 100) // 100)

    def __str__(self):
        fraction = self.cents % 100
        whole = (self.cents - fraction) // 100
        return str(whole) + "." + ("0" if fraction < 10 else "") + str(fraction)


class Product:
    __slots__ = ("id", "name", "category", "_price", "stock")

    def __init__(self, id, name, category, price, stock):
        self.id, self.name, self.category, self._price, self.stock = id, name, category, price, stock

    def price(self):
        return Money(self._price)


class Customer:
    __slots__ = ("id", "name", "tier")

    def __init__(self, id, name, tier):
        self.id, self.name, self.tier = id, name, tier


class Line:
    __slots__ = ("product", "quantity")

    def __init__(self, product, quantity):
        self.product, self.quantity = product, quantity

    def subtotal(self):
        return self.product.price().times(self.quantity)


class Order:
    __slots__ = ("id", "customer", "lines")

    def __init__(self, id, customer, lines=()):
        self.id, self.customer, self.lines = id, customer, list(lines)

    def items(self):
        return sum(line.quantity for line in self.lines)

    def subtotal(self):
        total = Money(0)
        for line in self.lines:
            total = total.plus(line.subtotal())
        return total

    def added(self, line):
        return Order(self.id, self.customer, self.lines + [line])


class OutOfStock(Exception):
    def __init__(self, product, available):
        super().__init__("product " + str(product) + " is out of stock")
        self.product, self.available = product, available


class Catalog:
    def __init__(self, size):
        self.size = size

    def product(self, id):
        return Product(id, "product-" + str(id), id * 7 % 10, id * 389 % 20000 + 199, id * 13 % 40 + 1)

    def customer(self, id):
        return Customer(id, "customer-" + str(id), id % 4)

    def reserved(self, product, quantity):
        if quantity > product.stock:
            raise OutOfStock(product.id, product.stock)
        return quantity


class BulkDiscount:
    name = "bulk"

    def adjusted(self, order, total):
        return total.minus(total.percent(5)) if total.cents > 200000 else total


class LoyaltyDiscount:
    name = "loyalty"

    def adjusted(self, order, total):
        return total.minus(total.percent(order.customer.tier * 2))


class CategoryPromotion:
    name = "promotion"

    def __init__(self, category):
        self.category = category

    def adjusted(self, order, total):
        discount = Money(0)
        for line in order.lines:
            if line.product.category == self.category:
                discount = discount.plus(line.subtotal().percent(10))
        return total.minus(discount)


class Tax:
    name = "tax"

    def adjusted(self, order, total):
        return total.plus(total.percent(20))


class PriceEngine:
    def __init__(self, rules):
        self.rules = rules

    def total(self, order):
        total = order.subtotal()
        for rule in self.rules:
            total = rule.adjusted(order, total)
        return total


class OrderSource:
    def __init__(self, catalog):
        self.catalog = catalog

    def order(self, id):
        catalog = self.catalog
        order = Order(id, catalog.customer(id * 31 % 5000))
        for index in range(id % 8 + 1):
            product = catalog.product((id * 97 + index * 1031) % catalog.size)
            wanted = (id + index * 3) % 12 + 1
            try:
                quantity = catalog.reserved(product, wanted)
            except OutOfStock as shortage:
                quantity = shortage.available
            order = order.added(Line(product, quantity))
        return order


NAMES = ["tools", "garden", "books", "music", "toys", "food", "sports", "office", "kitchen", "auto"]


class Report:
    __slots__ = ("revenue", "orders", "items", "cents")

    def __init__(self, revenue=None, orders=0, items=0, cents=0):
        self.revenue = revenue if revenue is not None else [0] * 10
        self.orders, self.items, self.cents = orders, items, cents

    def recorded(self, order, total):
        lines = order.lines
        revenue = [
            self.revenue[category]
            + sum(line.subtotal().cents for line in lines if line.product.category == category)
            for category in range(10)
        ]
        return Report(revenue, self.orders + 1, self.items + order.items(), self.cents + total.cents)

    def text(self):
        rows = [NAMES[c] + ": " + str(Money(self.revenue[c])) for c in range(10)]
        average = Money((self.cents - self.cents % self.orders) // self.orders)
        summary = [
            "orders: " + str(self.orders),
            "items: " + str(self.items),
            "revenue: " + str(Money(self.cents)),
            "average: " + str(average),
        ]
        return "\n".join(rows + summary)


class Quotes:
    def __init__(self, engine):
        self.engine = engine

    async def quote(self, order):
        return self.engine.total(order)


async def main():
    rules = [BulkDiscount(), CategoryPromotion(3), LoyaltyDiscount(), Tax()]
    quotes = Quotes(PriceEngine(rules))
    source = OrderSource(Catalog(2000))
    report = Report()
    for id in range(30000):
        order = source.order(id)
        total = await quotes.quote(order)
        report = report.recorded(order, total)
    text = report.text()
    print(text)
    print("length: " + str(len(text)))


asyncio.run(main())
