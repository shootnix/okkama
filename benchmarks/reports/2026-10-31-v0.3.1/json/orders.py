import json, sys, time
from decimal import Decimal
class Line:
    __slots__ = ("sku", "qty")
    def __init__(self, sku, qty): self.sku, self.qty = sku, qty
class Order:
    __slots__ = ("id", "name", "total", "paid", "lines")
    def __init__(self, id, name, total, paid, lines): self.id, self.name, self.total, self.paid, self.lines = id, name, total, paid, lines
def best(f, n=5):
    r = None; t = 1e9
    for _ in range(n):
        s = time.perf_counter(); r = f(); t = min(t, time.perf_counter() - s)
    return r, t * 1000
src = open(sys.argv[1]).read()
doc, tp = best(lambda: json.loads(src))
orders, td = best(lambda: [Order(r["id"], r["customer"]["name"], Decimal(r["total"]), r["paid"], [Line(l["sku"], l["qty"]) for l in r["lines"]]) for r in doc["orders"]])
out, tw = best(lambda: json.dumps({"orders": [{"id": o.id, "name": o.name, "total": str(o.total), "paid": o.paid, "lines": [{"sku": l.sku, "qty": l.qty} for l in o.lines]} for o in orders]}, separators=(",", ":")))
print("python  parse %6.1f ms  decode %6.1f ms  write %6.1f ms  total %6.1f ms" % (tp, td, tw, tp + td + tw))
