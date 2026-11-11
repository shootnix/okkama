#!/usr/bin/env python3
"""Run every example and compare its output with NAME.expected.

    examples/check.py [--engine ir|sparrow|jvm ...] [PATH ...]

Each example runs in a fresh temporary directory, with the std of this
checkout, on every engine given (default: ir and sparrow). Only the Python
standard library is used.
"""
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
OKKAMA = os.environ.get("OKKAMA", os.path.join(ROOT, "okkama-rs", "target", "release", "okkama"))

args = sys.argv[1:]
engines = []
while "--engine" in args:
    at = args.index("--engine")
    engines.append(args[at + 1])
    del args[at:at + 2]
engines = engines or ["ir", "sparrow"]
roots = [os.path.abspath(path) for path in args] or [HERE]
examples = sorted(
    os.path.join(folder, name)
    for root in roots
    for folder, _, names in os.walk(root)
    for name in names
    if name.endswith(".ok") and os.path.exists(os.path.join(folder, name[:-3] + ".expected"))
)
env = dict(os.environ, OKKAMA_STDLIB_DIR=os.path.join(ROOT, "std"))
failed = 0
for example in examples:
    expected = open(example[:-3] + ".expected", encoding="utf-8").read()
    for engine in engines:
        with tempfile.TemporaryDirectory() as scratch:
            run = subprocess.run([OKKAMA, "run", "--engine", engine, example], cwd=scratch, env=env,
                                 capture_output=True, text=True, timeout=60)
        ok = run.returncode == 0 and run.stdout == expected
        failed += not ok
        print("%-6s %-8s %s" % ("ok" if ok else "FAIL", engine, os.path.relpath(example, ROOT)))
        if not ok:
            print(run.stdout[-800:] + run.stderr[-800:])
print("%d examples, %d failed" % (len(examples) * len(engines), failed))
sys.exit(1 if failed else 0)
