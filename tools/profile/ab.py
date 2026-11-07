#!/usr/bin/env python3
"""A/B comparison of two Sparrow core binaries on one workload (#55).

    tools/profile/ab.py OLD/sparrow NEW/sparrow WORKLOAD.spb [--runs 15] [--seed N] [--out DIR]

Runs both binaries on the workload in a shuffled order, checks that their
output and exit status agree, and reports for each the median wall time with
its interquartile range, peak memory, and the differences of `--stats`
counters (allocations by site and kind, helpers, compile and collection
time) from one extra run each. An optimization is accepted when the new
binary's IQR lies below the old one's, the output is the same, and peak
memory does not grow by more than 5 %. With `--out`, everything — commit,
workload sha256, binaries' sha256, settings, samples, counters — is written
to DIR/ab.json so the comparison can be repeated.

Run it in the cage or Docker for workloads that spawn processes; the runs
are sequential. Uses only the Python standard library.
"""
import argparse
import hashlib
import json
import os
import platform
import random
import re
import statistics
import subprocess
import sys
import time
from pathlib import Path


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def run(binary, workload, extra=()):
    """One run: wall seconds, peak RSS bytes, exit status, stdout, stderr."""
    timer = ["/usr/bin/time", "-l"] if platform.system() == "Darwin" else ["/usr/bin/time", "-v"]
    start = time.perf_counter()
    done = subprocess.run([*timer, str(binary), "run", *extra, str(workload)], capture_output=True)
    wall = time.perf_counter() - start
    stderr = done.stderr.decode(errors="replace")
    rss = 0
    match = re.search(r"(\d+)\s+maximum resident set size", stderr) or re.search(r"Maximum resident set size \(kbytes\): (\d+)", stderr)
    if match:
        rss = int(match.group(1)) * (1 if platform.system() == "Darwin" else 1024)
    return {"wall": wall, "rss": rss, "exit": done.returncode, "stdout": done.stdout, "stderr": stderr}


def stats(binary, workload):
    """`--stats` counters of one run, by name."""
    counters = {}
    for line in run(binary, workload, ["--stats"])["stderr"].splitlines():
        match = re.match(r"\s*(\d+) (.+)$", line)
        if match and not re.match(r"\d+\s+(maximum|average|page|block|voluntary|involuntary|instructions|cycles|peak|messages|signals|swaps)", line.strip()):
            counters[match.group(2)] = int(match.group(1))
    return counters


def summary(values):
    values = sorted(values)
    q1, median, q3 = statistics.quantiles(values, n=4, method="inclusive") if len(values) > 1 else (values[0],) * 3
    return {"median": median, "q1": q1, "q3": q3, "min": values[0], "max": values[-1]}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("old", type=Path)
    parser.add_argument("new", type=Path)
    parser.add_argument("workload", type=Path)
    parser.add_argument("--runs", type=int, default=15)
    parser.add_argument("--seed", type=int, default=None)
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    seed = args.seed if args.seed is not None else int.from_bytes(os.urandom(4), "big")
    binaries = {"old": args.old, "new": args.new}
    for binary in binaries.values():
        run(binary, args.workload)  # warm-up: file and page caches
    order = [name for name in binaries for _ in range(args.runs)]
    random.Random(seed).shuffle(order)
    samples = {name: [] for name in binaries}
    outputs = {}
    for name in order:
        result = run(binaries[name], args.workload)
        outputs.setdefault(name, (result["exit"], result["stdout"]))
        if (result["exit"], result["stdout"]) != outputs[name]:
            sys.exit(f"{name}: output differs between its own runs")
        samples[name].append({"wall": result["wall"], "rss": result["rss"]})
    same = outputs["old"] == outputs["new"]
    wall = {name: summary([s["wall"] for s in runs]) for name, runs in samples.items()}
    rss = {name: max(s["rss"] for s in runs) for name, runs in samples.items()}
    counters = {name: stats(binary, args.workload) for name, binary in binaries.items()}
    changed = {key: (counters["old"].get(key, 0), counters["new"].get(key, 0))
               for key in sorted(set(counters["old"]) | set(counters["new"]))
               if counters["old"].get(key, 0) != counters["new"].get(key, 0)}

    faster = wall["new"]["q3"] < wall["old"]["q1"]
    slower = wall["new"]["q1"] > wall["old"]["q3"]
    memory_ok = rss["new"] <= rss["old"] * 1.05
    for name in binaries:
        w = wall[name]
        print(f"{name:3}  {w['median'] * 1000:9.1f} ms  IQR {w['q1'] * 1000:.1f}–{w['q3'] * 1000:.1f}  peak {rss[name] / 2**20:.1f} MB")
    print(f"output {'same' if same else 'DIFFERENT'}; time {'faster' if faster else 'slower' if slower else 'within spread'}; "
          f"memory {'ok' if memory_ok else 'GREW > 5 %'}")
    for key, (old, new) in list(changed.items())[:40]:
        print(f"  {old:>12} → {new:<12} {key}")
    verdict = same and faster and memory_ok
    print("accept" if verdict else "not accepted")
    if args.out:
        args.out.mkdir(parents=True, exist_ok=True)
        root = Path(__file__).resolve().parents[2]
        commit = subprocess.run(["git", "-C", str(root), "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip()
        record = {
            "commit": commit, "seed": seed, "runs": args.runs, "host": platform.platform(),
            "workload": {"path": str(args.workload), "sha256": sha256(args.workload)},
            "binaries": {name: {"path": str(path), "sha256": sha256(path)} for name, path in binaries.items()},
            "settings": {key: value for key, value in os.environ.items() if key.startswith(("SPARROW_", "OKKAMA_"))},
            "samples": samples, "wall": wall, "peak_rss": rss, "counters": counters,
            "same_output": same, "accepted": verdict,
        }
        (args.out / "ab.json").write_text(json.dumps(record, indent=2))
        print(f"record: {args.out / 'ab.json'}")
    sys.exit(0 if verdict else 1)


if __name__ == "__main__":
    main()
