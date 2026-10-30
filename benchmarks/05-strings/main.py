# Строки: сборка CSV на 30000 строк, split, замены, длины и разбор чисел.
def line(i):
    return "id=" + str(i) + ";name=item-" + str(i * 7 % 1000) + ";price=" + str(i * 13 % 9973)


def price(text):
    return int(text.split(";")[2].replace("price=", ""))


text = "\n".join(line(i) for i in range(30000))
lines = text.split("\n")
total = sum(price(row) for row in lines)
renamed = text.replace("item-", "product_")
print(len(lines), total, len(renamed), len(text))
