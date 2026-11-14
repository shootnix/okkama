#!/usr/bin/env python3
"""The t/ suite (#245): self-checking Okkama programs that print TAP.

    tools/t/run.py [--configs sparrow] [--jobs N] [--cage] [FILE|DIR ...]

Every file under t/ (or the ones given) runs on every configuration:

    sparrow         okkama run --engine sparrow
    sparrow-passes-off Sparrow with OKKAMA_IR_PASSES=none
    sparrow-gc-stress  Sparrow with SPARROW_CORE_GC_STRESS=1 (a collection
                    at every safepoint, freed cells poisoned)

Before the run, tools/t/gen.py writes the generated families to t/gen
(`--no-generate` keeps what is there). A file prints `1..N`, then `ok K - name` or `not ok K - name` for K = 1..N in
order; `# ...` lines are diagnostics. It fails on any `not ok`, a missing or
wrong plan, a check out of order, a non-zero exit, a budget stop, or a TAP
stream that differs from the first configuration's. Runs are bounded by
`--timeout` and `--max-memory`. `--cage` wraps sparrow runs in
okkama-rs/tools/new-runtime/caged.py for local use. Files import the modules of `t/support/` (the TAP helper
`tap.ok` and modules for t/lang) through OKKAMA_IMPORT_DIRS; they are not
tests themselves.

The summary follows `prove`: one line per file, then the failures and the
totals. Exit status 0 when every file passed on every configuration.
"""
import argparse
import concurrent.futures
import os
import re
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
SUITE = os.path.join(ROOT, "t")
SUPPORT = os.path.join(SUITE, "support")
OKKAMA = os.path.join(ROOT, "okkama-rs", "target", "release", "okkama")
CAGE = os.path.join(ROOT, "okkama-rs", "tools", "new-runtime", "caged.py")

CONFIGS = {
    "sparrow": ("sparrow", {}),
    "sparrow-passes-off": ("sparrow", {"OKKAMA_IR_PASSES": "none"}),
    "sparrow-gc-stress": ("sparrow", {"SPARROW_CORE_GC_STRESS": "1"}),
}

PLAN = re.compile(r"^1\.\.(\d+)$")
CHECK = re.compile(r"^(not )?ok (\d+)(?: - (.*))?$")


def files(paths):
    found = []
    for path in paths or [SUITE]:
        path = os.path.abspath(path)
        if os.path.isdir(path):
            for directory, _, names in os.walk(path):
                found += [os.path.join(directory, name) for name in names if name.endswith(".ok")]
        else:
            found.append(path)
    # Support modules are imported by the tests, not run.
    return sorted(path for path in found if not path.startswith(SUPPORT + os.sep))


def tap(stdout):
    """Problems of one TAP stream and the number of checks it ran."""
    lines = [line for line in stdout.splitlines() if line and not line.startswith("#")]
    if not lines or not PLAN.match(lines[0]):
        return ["no plan `1..N` on the first line"], 0
    planned = int(PLAN.match(lines[0]).group(1))
    problems, ran = [], 0
    for line in lines[1:]:
        check = CHECK.match(line)
        if not check:
            problems.append(f"not TAP: {line[:80]}")
            continue
        ran += 1
        if int(check.group(2)) != ran:
            problems.append(f"check {check.group(2)} out of order (expected {ran})")
        if check.group(1):
            problems.append(line)
    if ran != planned:
        problems.append(f"planned {planned} checks, ran {ran}")
    return problems, ran


def run(job, args):
    path, config = job
    engine, extra = CONFIGS[config]
    command = [args.okkama, "run", "--engine", engine, "--timeout", str(args.timeout),
               "--max-memory", args.max_memory, path]
    if args.cage:
        command = [sys.executable, CAGE, "--cpu-seconds", str(args.timeout), "--timeout", str(args.timeout + 30),
                   "--memory-mb", "2048", "--writable", os.path.dirname(path), "--"] + command
    env = dict(os.environ, OKKAMA_IMPORT_DIRS=SUPPORT, OKKAMA_STDLIB_DIR=os.path.join(ROOT, "std"),
               OKKAMA_TRACE="off", **extra)
    try:
        result = subprocess.run(command, cwd=os.path.dirname(path), env=env, capture_output=True, text=True,
                                timeout=args.timeout + 60)
        stdout, code, stderr = result.stdout, result.returncode, result.stderr
    except subprocess.TimeoutExpired:
        stdout, code, stderr = "", None, "timeout"
    problems, ran = tap(stdout)
    if code != 0:
        problems.append(f"exit {code}: {stderr.strip().splitlines()[-1][:160] if stderr.strip() else ''}")
    return path, config, stdout, problems, ran


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("paths", nargs="*")
    parser.add_argument("--configs", default="sparrow")
    parser.add_argument("--jobs", type=int, default=max(1, (os.cpu_count() or 2) // 2))
    parser.add_argument("--okkama", default=OKKAMA)
    parser.add_argument("--timeout", type=int, default=60)
    parser.add_argument("--max-memory", default="1G")
    parser.add_argument("--cage", action="store_true")
    parser.add_argument("--verbose", "-v", action="store_true", help="print every failing stream")
    parser.add_argument("--no-generate", action="store_true", help="keep t/gen as it is (tools/t/gen.py writes it)")
    args = parser.parse_args()
    configs = [config for config in args.configs.split(",") if config]
    unknown = [config for config in configs if config not in CONFIGS]
    if unknown:
        parser.error(f"unknown configurations: {', '.join(unknown)} (known: {', '.join(CONFIGS)})")
    # Programs run from their own directory: the binary needs an absolute path.
    args.okkama = os.path.abspath(args.okkama)
    if not os.access(args.okkama, os.X_OK):
        parser.error(f"no okkama binary at {args.okkama} (build it or pass --okkama)")
    if not args.no_generate:
        subprocess.run([sys.executable, os.path.join(HERE, "gen.py")], check=True, stdout=subprocess.DEVNULL)
    tests = files(args.paths)
    started = time.time()
    results = {}
    with concurrent.futures.ThreadPoolExecutor(args.jobs) as pool:
        for path, config, stdout, problems, ran in pool.map(lambda job: run(job, args),
                                                            [(path, config) for path in tests for config in configs]):
            results[(path, config)] = (stdout, problems, ran)

    width = max((len(os.path.relpath(path, ROOT)) for path in tests), default=0) + 4
    failed, checks = [], 0
    for path in tests:
        name = os.path.relpath(path, ROOT)
        reference = results[(path, configs[0])][0]
        problems = []
        for config in configs:
            stdout, found, ran = results[(path, config)]
            problems += [f"{config}: {problem}" for problem in found]
            if stdout != reference:
                problems.append(f"{config}: TAP differs from {configs[0]}")
        checks += results[(path, configs[0])][2]
        print(f"{name} {'.' * (width - len(name))} {'ok' if not problems else 'FAILED'}")
        if problems:
            failed.append((name, problems))
            for problem in problems[:12]:
                print(f"    {problem}")
            if args.verbose:
                for config in configs:
                    print(f"  --- {config}\n{results[(path, config)][0]}")
    elapsed = time.time() - started
    print()
    if failed:
        print(f"Failed {len(failed)}/{len(tests)} files: {', '.join(name for name, _ in failed)}")
    else:
        print("All tests successful.")
    print(f"Files={len(tests)}, Tests={checks}, Configs={len(configs)} ({', '.join(configs)}), {elapsed:.1f} s")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
