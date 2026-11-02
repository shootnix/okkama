#!/usr/bin/env python3
"""JSON benchmark: every document shape × every implementation.

usage: run.py [--runs N] [--only shape,...] [--engines name,...] [--data DIR] [--out FILE]

Implementations: Okkama on Sparrow and on the JVM (walk.ok), Python 3 (walk.py,
its C json module), Perl (walk.pl, pure-Perl JSON::PP). Each run prints the
parse, walk and write times measured inside the program and a checksum; the
runner checks that all implementations agree on it and reports medians.
"""
import argparse, json, os, statistics, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
OKKAMA = os.path.join(ROOT, "okkama-rs", "target", "release", "okkama")
ENGINES = {
    "okkama": [OKKAMA, "run", "--engine", "sparrow", os.path.join(HERE, "walk.ok")],
    "okkama-jvm": [OKKAMA, "run", "--engine", "jvm", os.path.join(HERE, "walk.ok")],
    "python": [sys.executable, "-I", os.path.join(HERE, "walk.py")],
    "perl": ["perl", os.path.join(HERE, "walk.pl")],
}
SHAPES = ["records", "wide", "deep", "strings", "numbers", "orders"]
CHECKSUM = ("nodes", "chars", "numbers", "trues")


def run_once(engine, path):
    command = ENGINES[engine] + ([] if engine.startswith("okkama") else [path])
    env = dict(os.environ, JSON_FILE=path, OKKAMA_STDLIB_DIR=os.path.join(ROOT, "std"))
    out = subprocess.run(command, env=env, capture_output=True, text=True, check=True).stdout
    return json.loads(out.strip().splitlines()[-1])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--only", default=",".join(SHAPES))
    parser.add_argument("--engines", default=",".join(ENGINES))
    parser.add_argument("--data")
    parser.add_argument("--out")
    args = parser.parse_args()
    data = args.data or os.path.join(tempfile.gettempdir(), "okkama-json-bench")
    if not all(os.path.exists(os.path.join(data, s + ".json")) for s in SHAPES):
        subprocess.run([sys.executable, "-I", os.path.join(HERE, "generate.py"), data], check=True)
    results = {}
    for shape in args.only.split(","):
        path = os.path.join(data, shape + ".json")
        results[shape] = {"bytes": os.path.getsize(path)}
        for engine in args.engines.split(","):
            runs = [run_once(engine, path) for _ in range(args.runs)]
            summary = {phase: round(statistics.median(r[phase] for r in runs), 1) for phase in ("parse", "walk", "write")}
            summary["checksum"] = [runs[0][k] for k in CHECKSUM]
            results[shape][engine] = summary
            print(f"{shape:8} {engine:11} parse {summary['parse']:8.1f}  walk {summary['walk']:8.1f}  "
                  f"write {summary['write']:8.1f} ms", file=sys.stderr)
        sums = {tuple(v["checksum"]) for k, v in results[shape].items() if k != "bytes"}
        if len(sums) != 1:
            sys.exit(f"{shape}: implementations disagree on the checksum: {sums}")
    if args.out:
        with open(args.out, "w") as fh:
            json.dump(results, fh, indent=1)
    engines = args.engines.split(",")
    print("| shape | size | phase | " + " | ".join(engines) + " |")
    print("|---|---:|---|" + "---:|" * len(engines))
    for shape, by_engine in results.items():
        for phase in ("parse", "walk", "write"):
            cells = " | ".join(f"{by_engine[e][phase]:.1f}" for e in engines)
            size = f"{by_engine['bytes'] / 1e6:.1f} MB" if phase == "parse" else ""
            print(f"| {shape if phase == 'parse' else ''} | {size} | {phase} | {cells} |")


if __name__ == "__main__":
    main()
