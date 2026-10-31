#!/usr/bin/env python3
"""JSON documents of several shapes for the JSON benchmark.

usage: generate.py <output directory>

  records  100 000 small objects (many small allocations)
  wide     5 objects of 20 000 members each (few, very large objects)
  deep     2 000 chains nested 100 levels (deep recursion)
  strings  20 000 long strings with escapes, Cyrillic and emoji
  numbers  300 000 numbers: small and large integers, fractions, exponents
  orders   10 000 orders of the 0.3.1 report (nested objects and arrays)

The documents are deterministic: the same script writes the same bytes.
"""
import json, os, random, sys


def records(rng):
    return [{"id": i, "name": "user-%d" % i, "email": "user%d@example.com" % i,
             "active": i % 3 != 0, "score": round(rng.uniform(0, 100), 2),
             "tags": ["t%d" % (i % 7), "g%d" % (i % 11)],
             "address": {"city": "City %d" % (i % 500), "zip": "%05d" % (i % 99999)}}
            for i in range(100_000)]


def wide(rng):
    return [{"k%05d" % j: (j if j % 2 == 0 else "value-%d-%d" % (i, j)) for j in range(20_000)}
            for i in range(5)]


def deep(rng):
    chains = []
    for i in range(2_000):
        value = "leaf-%d" % i
        for level in range(100):
            value = {"a": value, "n": level} if level % 2 == 0 else [value, level]
        chains.append(value)
    return chains


def strings(rng):
    pieces = ["plain text ", "quote \" ", "back\\slash ", "new\nline ", "tab\t ", "é ", "Привет ",
              "🙂 ", "\u0001 ", "/ ", "long-word-without-escapes "]
    return ["".join(rng.choice(pieces) for _ in range(25)) for _ in range(20_000)]


def numbers(rng):
    out = []
    for i in range(300_000):
        kind = i % 5
        if kind == 0:
            out.append(rng.randint(-1000, 1000))
        elif kind == 1:
            out.append(rng.randint(10**12, 10**15))
        elif kind == 2:
            out.append(round(rng.uniform(-1e4, 1e4), 2))
        elif kind == 3:
            out.append(float("%.3e" % rng.uniform(1, 1e12)))
        else:
            out.append(rng.randint(0, 9))
    return out


def orders(rng):
    rng = random.Random(1)
    data = None
    for count in (1000, 10000):
        data = {"orders": [{"id": i, "customer": {"name": "Customer %d" % i, "email": "c%d@example.com" % i},
                            "total": "%d.%02d" % (rng.randint(1, 9999), rng.randint(0, 99)),
                            "paid": i % 3 == 0, "tags": ["a", "b", "c"][: i % 4],
                            "lines": [{"sku": "SKU-%d" % j, "qty": j + 1, "price": 12.5} for j in range(3)]}
                           for i in range(count)]}
    return data


SHAPES = {"records": records, "wide": wide, "deep": deep, "strings": strings, "numbers": numbers, "orders": orders}

if __name__ == "__main__":
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    for name, make in SHAPES.items():
        path = os.path.join(out, name + ".json")
        with open(path, "w", encoding="utf-8") as fh:
            json.dump(make(random.Random(7)), fh, ensure_ascii=False)
        print("%-8s %10d bytes" % (name, os.path.getsize(path)))
