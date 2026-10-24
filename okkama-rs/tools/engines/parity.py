#!/usr/bin/env python3
"""Engine parity: IR interpreter and Sparrow against the JVM path.

Every std file with tests runs in test mode (`okkama run -t`) and every
example with `main` runs normally, once per engine. The JVM result is the
reference: an engine matches when the exit code, stdout and stderr are equal.
A JVM result that differs between two JVM runs is nondeterministic and is
reported separately instead of as a divergence.

    cargo build -p okkama-cli -p okkama-native
    tools/engines/parity.py [--engines ir,sparrow] [--set std,examples]
                            [-j 8] [--filter SUBSTRING] [--refresh]

The JVM baseline is cached in target/parity/jvm.json by file content and the
std tree digest; --refresh recomputes it. Reports: target/parity/report.json
and report.md. Only the Python standard library is used.
"""

import argparse
import concurrent.futures
import hashlib
import json
import os
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
RUST_ROOT = HERE.parent.parent
REPO_ROOT = RUST_ROOT.parent
OUT = RUST_ROOT / "target" / "parity"
BINARY = RUST_ROOT / "target" / "debug" / "okkama"

TEST_RE = re.compile(r'^\s*(async\s+)?test\s+"', re.M)
MAIN_RE = re.compile(r'^\s*(async\s+)?main\s*\(', re.M)


def discover(sets, needle):
    cases = []
    if "std" in sets:
        for path in sorted((REPO_ROOT / "std").rglob("*.ok")):
            if TEST_RE.search(path.read_text(encoding="utf-8", errors="replace")):
                cases.append(("std", path, True))
    if "examples" in sets:
        for path in sorted((REPO_ROOT / "examples").rglob("*.ok")):
            if MAIN_RE.search(path.read_text(encoding="utf-8", errors="replace")):
                cases.append(("examples", path, False))
    if needle:
        cases = [case for case in cases if needle in str(case[1])]
    return cases


def tree_digest(root):
    digest = hashlib.sha256()
    for path in sorted(root.rglob("*.ok")):
        digest.update(str(path.relative_to(root)).encode())
        digest.update(path.read_bytes())
    return digest.hexdigest()[:16]


def run(engine, path, tests, timeout):
    command = [str(BINARY), "run", "--engine", engine]
    if tests:
        command.append("-t")
    command.append(str(path))
    try:
        done = subprocess.run(
            command,
            cwd=path.parent,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired:
        return {"code": "timeout", "stdout": "", "stderr": ""}
    return {
        "code": done.returncode,
        "stdout": done.stdout.decode("utf-8", "replace"),
        "stderr": done.stderr.decode("utf-8", "replace"),
    }


def same(left, right):
    return all(left[key] == right[key] for key in ("code", "stdout", "stderr"))


def classify(result):
    text = result["stderr"]
    for marker in (
        "native provider is not implemented",
        "is not supported",
        "not supported yet",
        "invalid interpreter state",
        "Sparrow codegen",
    ):
        if marker in text:
            line = next((l for l in text.splitlines() if marker in l), text)
            return "unsupported", line.strip()[:200]
    if result["code"] == "timeout":
        return "timeout", ""
    return "diverged", ""


def first_difference(reference, result):
    for key in ("code", "stdout", "stderr"):
        if reference[key] != result[key]:
            if key == "code":
                return f"exit {reference['code']} vs {result['code']}"
            a = reference[key].splitlines()
            b = result[key].splitlines()
            for index in range(max(len(a), len(b))):
                x = a[index] if index < len(a) else "<end>"
                y = b[index] if index < len(b) else "<end>"
                if x != y:
                    return f"{key} line {index + 1}: jvm {x[:120]!r} vs {y[:120]!r}"
    return ""


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--engines", default="ir,sparrow")
    parser.add_argument("--set", default="std,examples")
    parser.add_argument("-j", type=int, default=8)
    parser.add_argument("--filter", default="")
    parser.add_argument("--refresh", action="store_true")
    parser.add_argument("--timeout", type=int, default=60)
    args = parser.parse_args()
    if not BINARY.is_file():
        sys.exit(f"build the CLI first: {BINARY} is missing")
    # Без хостовой криптографии JVM-эталон std/crypto, std/digest и
    # std/db/postgres падает, и это выглядит как расхождение движков.
    if not any(BINARY.parent.glob("libokkama_native.*")):
        sys.exit("build the native library too: cargo build -p okkama-cli -p okkama-native")
    engines = [engine for engine in args.engines.split(",") if engine]
    cases = discover(set(args.set.split(",")), args.filter)
    OUT.mkdir(parents=True, exist_ok=True)
    cache_path = OUT / "jvm.json"
    cache = {} if args.refresh or not cache_path.is_file() else json.loads(cache_path.read_text())
    std_digest = tree_digest(REPO_ROOT / "std")

    def key(path):
        return f"{std_digest}:{hashlib.sha256(path.read_bytes()).hexdigest()[:16]}:{path}"

    def job(case):
        group, path, tests = case
        reference = cache.get(key(path))
        if reference is None:
            reference = run("jvm", path, tests, args.timeout)
        outcome = {"group": group, "path": str(path.relative_to(REPO_ROOT)), "jvm": reference}
        for engine in engines:
            result = run(engine, path, tests, args.timeout)
            if same(reference, result):
                outcome[engine] = {"status": "match"}
                continue
            again = run("jvm", path, tests, args.timeout)
            if not same(reference, again):
                outcome[engine] = {"status": "nondeterministic"}
                continue
            status, reason = classify(result)
            outcome[engine] = {
                "status": status,
                "reason": reason,
                "difference": first_difference(reference, result),
            }
        return path, outcome

    results = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.j) as pool:
        for index, (path, outcome) in enumerate(pool.map(job, cases), 1):
            cache[key(path)] = outcome["jvm"]
            marks = " ".join(f"{e}={outcome[e]['status']}" for e in engines)
            print(f"[{index:4}/{len(cases)}] {outcome['path']}: {marks}", flush=True)
            results.append(outcome)
    cache_path.write_text(json.dumps(cache))

    summary = {}
    for engine in engines:
        counts = {}
        for outcome in results:
            status = outcome[engine]["status"]
            counts[status] = counts.get(status, 0) + 1
        summary[engine] = counts
    (OUT / "report.json").write_text(json.dumps({"summary": summary, "results": results}, indent=1))
    lines = ["# Engine parity", ""]
    for engine in engines:
        counts = ", ".join(f"{k} {v}" for k, v in sorted(summary[engine].items()))
        lines.append(f"- **{engine}**: {counts} (of {len(results)})")
    for engine in engines:
        lines += ["", f"## {engine}: not matching", ""]
        for outcome in results:
            entry = outcome[engine]
            if entry["status"] == "match":
                continue
            detail = entry.get("reason") or entry.get("difference") or ""
            lines.append(f"- `{outcome['path']}` — {entry['status']}: {detail}")
    (OUT / "report.md").write_text("\n".join(lines) + "\n")
    for engine in engines:
        print(engine, summary[engine])


if __name__ == "__main__":
    main()
