#!/usr/bin/env python3
"""Orders documents of the JSON benchmark: generate.py <1000|10000|50000> > orders-<count>.json

One random stream draws the three sizes in order, as for the published runs.
"""
import json, random, sys
random.seed(1)
wanted = int(sys.argv[1])
for count in (1000, 10000, 50000):
    orders = [{"id": i, "customer": {"name": "Customer %d" % i, "email": "c%d@example.com" % i},
               "total": "%d.%02d" % (random.randint(1, 9999), random.randint(0, 99)),
               "paid": i % 3 == 0, "tags": ["a", "b", "c"][: i % 4],
               "lines": [{"sku": "SKU-%d" % j, "qty": j + 1, "price": 12.5} for j in range(3)]}
              for i in range(count)]
    if count == wanted:
        sys.stdout.write(json.dumps({"orders": orders}))
        break
