# Коллекции: map/filter/reduce, сортировка, словари и частоты.
from collections import Counter

values = [(i * 7919 + 13) % 10007 for i in range(200000)]
evens = [n for n in values if n % 2 == 0]
squares = [n * n % 1000003 for n in evens]
total = sum(squares)
ordered = sorted(values[:50000])
buckets = Counter(n % 97 for n in values[:4000])
index = {"low": len([n for n in values if n < 5000]), "high": len([n for n in values if n >= 5000])}
print(total, ordered[0], ordered[49999], len(buckets), buckets[0], index["low"], index["high"])
