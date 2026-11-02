#!/usr/bin/env python3
"""JSON benchmark in Python: parse, walk and write one document.

usage: walk.py <file>  — prints one JSON line with the phases in ms and the checksum.
"""
import json, sys, time


def walk(value, sums):
    sums[0] += 1
    if isinstance(value, dict):
        for name, item in value.items():
            sums[1] += len(name)
            walk(item, sums)
    elif isinstance(value, list):
        for item in value:
            walk(item, sums)
    elif isinstance(value, str):
        sums[1] += len(value)
    elif isinstance(value, bool):
        sums[3] += value
    elif value is not None:
        sums[2] += 1


with open(sys.argv[1], encoding="utf-8") as fh:
    text = fh.read()
start = time.perf_counter()
document = json.loads(text)
parsed = time.perf_counter()
sums = [0, 0, 0, 0]
walk(document, sums)
walked = time.perf_counter()
out = json.dumps(document, separators=(",", ":"), ensure_ascii=False)
written = time.perf_counter()
print(json.dumps({"parse": (parsed - start) * 1000, "walk": (walked - parsed) * 1000,
                  "write": (written - walked) * 1000, "nodes": sums[0], "chars": sums[1],
                  "numbers": sums[2], "trues": sums[3], "length": len(out)}))
