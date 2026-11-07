#!/usr/bin/env python3
"""Compiler phases on very large functions (H-28, H-39, #50).

    tools/gen/large.py [--okkama BIN] [--sizes 1000,10000,100000] [--shapes lets,literal,branches,loops] [--limit 30]

Generates one `main` per shape and size — many `let`s, one long array
literal, many `if` statements, many sequential loops — and times the
compiler's commands on it: `check` (parse and analysis), `ir --passes none`
(+ lowering), `ir` (+ IR passes) and `build --target sparrow` (+ codegen and
SPB verification). A phase's cost is the difference of consecutive commands.
For every tenfold step of the size it prints the growth of each phase; a
growth above `--limit` (n log n gives about 12, n² 100) is reported and makes
the exit status 1.

Each command runs once per size with a timeout; run it where long builds do
not disturb other work. Uses only the Python standard library.
"""
import argparse
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HEADER = "module app v1.0.0;\n\n"


def lets(n):
    body = ["    let v0 = 1;"] + [f"    let v{i} = v{i - 1} + {i % 7};" for i in range(1, n)]
    return HEADER + "main() is int {\n" + "\n".join(body) + f"\n    v{n - 1} % 256;\n}}\n"


def literal(n):
    items = ", ".join(str(i % 97) for i in range(n))
    return HEADER + f"main() is int {{\n    let items = [{items}];\n    items.size() % 256;\n}}\n"


def branches(n):
    body = ["    let x = 3;"] + [f"    if x == {i} {{ return {i % 200}; }}" for i in range(n)]
    return HEADER + "main() is int {\n" + "\n".join(body) + "\n    0;\n}\n"


def loops(n):
    count = max(1, n // 4)
    body = ["    let total = 0;"]
    body += [f"    let s{i} = (0..3).reduce(0) -> sum, k {{ sum + k + {i % 5}; }};" for i in range(count)]
    return HEADER + "main() is int {\n" + "\n".join(body) + "\n    s0 % 256;\n}\n"


SHAPES = {"lets": lets, "literal": literal, "branches": branches, "loops": loops}
COMMANDS = [
    ("parse+analysis", ["check"]),
    ("lowering", ["ir", "--passes", "none"]),
    ("passes", ["ir"]),
    ("codegen", ["build", "--target", "sparrow", "-o", "{out}"]),
]


def timed(okkama, args, path, out, timeout):
    command = [okkama] + [a.replace("{out}", str(out)) for a in args] + [str(path)]
    start = time.perf_counter()
    try:
        done = subprocess.run(command, capture_output=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        return None, "timeout"
    elapsed = time.perf_counter() - start
    if done.returncode != 0:
        return None, done.stderr.decode(errors="replace")[:300]
    return elapsed, None


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--okkama", default=os.environ.get("OKKAMA", "okkama-rs/target/release/okkama"))
    parser.add_argument("--sizes", default="1000,10000,100000")
    parser.add_argument("--shapes", default=",".join(SHAPES))
    parser.add_argument("--limit", type=float, default=30.0)
    parser.add_argument("--timeout", type=int, default=600)
    args = parser.parse_args()
    sizes = [int(s) for s in args.sizes.split(",")]
    worst = []
    with tempfile.TemporaryDirectory() as scratch:
        for shape in args.shapes.split(","):
            phases = {}
            for n in sizes:
                path = Path(scratch) / f"{shape}-{n}.ok"
                path.write_text(SHAPES[shape](n))
                totals = []
                for name, command in COMMANDS:
                    seconds, error = timed(args.okkama, command, path, Path(scratch) / "out.spb", args.timeout)
                    if error:
                        print(f"{shape:9} n={n:<7} {name:15} failed: {error.strip()}")
                        totals.append(None)
                        break
                    totals.append(seconds)
                previous = 0.0
                for (name, _), total in zip(COMMANDS, totals):
                    if total is None:
                        break
                    phases.setdefault(name, []).append((n, max(total - previous, 0.0)))
                    previous = total
                print(f"{shape:9} n={n:<7} " + "  ".join(
                    f"{name} {cost:7.3f}s" for name, runs in phases.items() for (m, cost) in runs if m == n), flush=True)
            for name, runs in phases.items():
                for (a, ta), (b, tb) in zip(runs, runs[1:]):
                    # Below 50 ms a phase is process noise, not growth.
                    if tb < 0.05:
                        continue
                    growth = tb / max(ta, 0.005)
                    flag = "  <-- superlinear" if growth > args.limit * (b / a) / 10 else ""
                    print(f"  {shape:9} {name:15} n {a}→{b}: ×{growth:.1f}{flag}")
                    if flag:
                        worst.append((shape, name, a, b, growth))
    if worst:
        print("superlinear phases:", ", ".join(f"{s}/{p} ×{g:.0f} at {a}→{b}" for s, p, a, b, g in worst))
        sys.exit(1)


if __name__ == "__main__":
    main()
