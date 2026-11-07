#!/usr/bin/env python3
"""Generated Okkama programs compared across engines (hardening H-18).

    tools/gen/okgen.py run [--seeds N] [--first S] [--engines ir,sparrow,jvm] [--out DIR]
    tools/gen/okgen.py show SEED

Each seed makes one terminating, deterministic program: exact integers with
BigInt promotion, strings, arrays, objects, a role with two implementations,
closures, try/catch over an error enum and bounded recursion. Every statement
of `main` prints one value, so engines are compared line by line. A seed whose
engines disagree is minimized (delta debugging over the statements of
`main`) and kept in DIR/<seed>/: the program, the minimized program and every
engine's output. Exit status 1 when some seed disagreed.

Programs run through `okkama run --engine E` (OKKAMA, default the release
build); a whole campaign belongs in a container (`make gate`'s image) or CI.
"""
import argparse
import os
import random
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
OKKAMA = os.environ.get("OKKAMA", os.path.join(ROOT, "okkama-rs", "target", "release", "okkama"))

PRELUDE = """module app;
import "io/stdout" take Stdout;

enum Fault is Error = (fault(code is int));

role Shape {
    area() is int;
    name() is string;
}

object Square does Shape {
    has side is int;
    Square(side is int) { self.side = side; }
    area() is int { side * side; }
    name() is string { "square-" + side.string(); }
}

object Strip does Shape {
    has width is int;
    has length is int;
    Strip(width is int, length is int) { self.width = width; self.length = length; }
    area() is int { width * length; }
    name() is string { "strip-" + width.string() + "x" + length.string(); }
}

object Lab {
    Lab();
    power(base is int, exponent is int) is int {
        if exponent == 0 { 1; } else { base * self.power(base, exponent - 1); }
    }
    sum(n is int) is int { if n == 0 { 0; } else { n + self.sum(n - 1); } }
    measure(shape is Shape) is int { shape.area() + shape.name().length(); }
    checked(value is int) is int {
        if value % 7 == 3 { throw Fault.fault(value); }
        value * 2;
    }
    guarded(value is int) is int {
        try { self.checked(value); } catch Fault.fault(code) { 0 - code; }
    }
    adder(amount is int) is (int) is int { (value is int) is int { value + amount; }; }
    twice(step is (int) is int, value is int) is int { step(step(value)); }
}
"""


class Gen:
    """Expressions of one type from a seeded random source."""

    def __init__(self, seed):
        self.random = random.Random(seed)

    def small(self):
        return self.random.randint(-50, 50)

    def big(self):
        digits = self.random.randint(15, 40)
        text = str(self.random.randint(1, 9)) + "".join(str(self.random.randint(0, 9)) for _ in range(digits - 1))
        return ("-" if self.random.random() < 0.3 else "") + text

    def int_expr(self, depth=0):
        r = self.random.random()
        if depth > 2 or r < 0.25:
            return str(self.small()) if self.random.random() < 0.7 else self.big()
        if r < 0.55:
            op = self.random.choice(["+", "-", "*"])
            return f"({self.int_expr(depth + 1)} {op} {self.int_expr(depth + 1)})"
        if r < 0.65:
            divisor = self.random.choice([3, 7, -5, 11, 1000000007])
            return f"({self.int_expr(depth + 1)} % {divisor})"
        if r < 0.72:
            return f"lab.power({self.random.randint(-9, 9)}, {self.random.randint(0, 30)})"
        if r < 0.78:
            return f"lab.sum({self.random.randint(0, 60)})"
        if r < 0.84:
            return f"lab.guarded({self.int_expr(depth + 1)})"
        if r < 0.9:
            return f"lab.twice(lab.adder({self.small()}), {self.int_expr(depth + 1)})"
        if r < 0.95:
            return f"lab.measure({self.shape()})"
        return f"{self.array_expr()}.size()"

    def shape(self):
        if self.random.random() < 0.5:
            return f"Square({self.random.randint(0, 40)})"
        return f"Strip({self.random.randint(0, 40)}, {self.random.randint(0, 40)})"

    def array_expr(self):
        n = self.random.randint(0, 6)
        return "[" + ", ".join(str(self.small()) for _ in range(n)) + "]"

    def string_expr(self, depth=0):
        r = self.random.random()
        if depth > 1 or r < 0.3:
            word = "".join(self.random.choice("abcxyz") for _ in range(self.random.randint(0, 5)))
            return f'"{word}"'
        if r < 0.6:
            return f"({self.string_expr(depth + 1)} + {self.string_expr(depth + 1)})"
        if r < 0.8:
            return f"{self.int_expr(depth + 1)}.string()"
        return f"{self.shape()}.name()"

    def statement(self):
        r = self.random.random()
        if r < 0.45:
            return f"out.writeln({self.int_expr()}.string());"
        if r < 0.65:
            return f"out.writeln({self.string_expr()});"
        if r < 0.8:
            array = self.array_expr()
            return f"out.writeln({array}.map()->x{{ x * {self.small()} + {self.small()}; }}.reduce(0)->a,b{{ a + b; }}.string());"
        if r < 0.9:
            return f"out.writeln(({self.int_expr()} < {self.int_expr()}).string());"
        return f"out.writeln(({self.string_expr()}).length().string());"


def program(seed, statements=None):
    gen = Gen(seed)
    body = [gen.statement() for _ in range(gen.random.randint(8, 24))]
    if statements is not None:
        body = [body[i] for i in statements]
    lines = "\n".join("    " + line for line in body)
    return PRELUDE + f"\nmain() is int {{\n    let out = Stdout();\n    let lab = Lab();\n{lines}\n    0;\n}}\n", len(body)


def run(text, engine, directory):
    path = os.path.join(directory, "main.ok")
    with open(path, "w") as out:
        out.write(text)
    try:
        p = subprocess.run([OKKAMA, "run", "--engine", engine, path], capture_output=True, text=True, timeout=120, cwd=directory)
        return (p.returncode, p.stdout, p.stderr.strip().splitlines()[:1])
    except subprocess.TimeoutExpired:
        return ("timeout", "", [])


def outcomes(text, engines):
    with tempfile.TemporaryDirectory() as directory:
        return {engine: run(text, engine, directory) for engine in engines}


def disagree(results):
    # Compare exit status and stdout; the first stderr line names a failure.
    seen = {(r[0], r[1], tuple(r[2]) if r[0] not in (0, "timeout") else ()) for r in results.values()}
    return len(seen) > 1


def minimize(seed, count, engines):
    """Delta debugging (ddmin) over the statements of main."""
    keep = list(range(count))
    chunks = 2
    while len(keep) >= 2:
        size = max(1, len(keep) // chunks)
        reduced = False
        for start in range(0, len(keep), size):
            candidate = keep[:start] + keep[start + size:]
            if candidate and disagree(outcomes(program(seed, candidate)[0], engines)):
                keep = candidate
                chunks = max(chunks - 1, 2)
                reduced = True
                break
        if not reduced:
            if size == 1:
                break
            chunks = min(len(keep), chunks * 2)
    return program(seed, keep)[0]


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    runner = sub.add_parser("run")
    runner.add_argument("--seeds", type=int, default=50)
    runner.add_argument("--first", type=int, default=1)
    runner.add_argument("--engines", default="ir,sparrow,jvm")
    runner.add_argument("--out", default=os.path.join(ROOT, "conformance", "reports", "runs", "okgen"))
    show = sub.add_parser("show")
    show.add_argument("seed", type=int)
    args = parser.parse_args()
    if args.command == "show":
        print(program(args.seed)[0], end="")
        return 0
    engines = args.engines.split(",")
    found = 0
    for seed in range(args.first, args.first + args.seeds):
        text, count = program(seed)
        results = outcomes(text, engines)
        if not disagree(results):
            continue
        found += 1
        keep = os.path.join(args.out, str(seed))
        os.makedirs(keep, exist_ok=True)
        with open(os.path.join(keep, "main.ok"), "w") as out:
            out.write(text)
        small = minimize(seed, count, engines)
        with open(os.path.join(keep, "minimized.ok"), "w") as out:
            out.write(small)
        for engine, (status, stdout, stderr) in outcomes(small, engines).items():
            with open(os.path.join(keep, f"{engine}.txt"), "w") as out:
                out.write(f"status: {status}\n{''.join(stderr)}\n---\n{stdout}")
        print(f"seed {seed}: engines disagree; minimized to {small.count('out.writeln')} statements in {keep}")
    print(f"{args.seeds} seeds, {found} disagreements")
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())
