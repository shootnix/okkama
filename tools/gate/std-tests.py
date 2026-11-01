#!/usr/bin/env python3
"""Встроенные тесты всех модулей std на движках IR и Sparrow.

usage: std-tests.py [--jobs N]

Модуль с тестами — файл std/ со строкой, начинающейся с `test "`. Каждый
запускается `okkama run --engine E -t`; провал — ненулевой код выхода.
Известные провалы перечислены в known-failures.txt рядом: они печатаются,
но не роняют проверку; новый провал или исправленный известный — роняют.
"""
import argparse, concurrent.futures, os, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
OKKAMA = os.path.join(ROOT, "okkama-rs", "target", "release", "okkama")
ENGINES = ("ir", "sparrow")


def known():
    entries = set()
    with open(os.path.join(HERE, "known-failures.txt"), encoding="utf-8") as lines:
        for line in lines:
            line = line.split("#", 1)[0].strip()
            if line:
                entries.add(line)
    return entries


def modules():
    found = []
    for directory, _, files in os.walk(os.path.join(ROOT, "std")):
        for name in files:
            path = os.path.join(directory, name)
            if name.endswith(".ok") and any(line.startswith('test "') for line in open(path, encoding="utf-8")):
                found.append(os.path.relpath(path, ROOT))
    return sorted(found)


def run(job):
    path, engine = job
    env = dict(os.environ, OKKAMA_STDLIB_DIR=os.path.join(ROOT, "std"))
    try:
        result = subprocess.run([OKKAMA, "run", "--engine", engine, "-t", path], cwd=ROOT, env=env,
                                capture_output=True, text=True, timeout=600)
        return path, engine, result.returncode == 0, (result.stdout + result.stderr).strip().splitlines()[-1:]
    except subprocess.TimeoutExpired:
        return path, engine, False, ["timeout"]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--jobs", type=int, default=6)
    args = parser.parse_args()
    expected = known()
    jobs = [(path, engine) for path in modules() for engine in ENGINES]
    failed = {}
    with concurrent.futures.ThreadPoolExecutor(args.jobs) as pool:
        for path, engine, passed, tail in pool.map(run, jobs):
            if not passed:
                failed[f"{engine} {path}"] = tail[0] if tail else ""
    new = sorted(set(failed) - expected)
    fixed = sorted(expected - set(failed))
    print(f"std: {len(jobs) // len(ENGINES)} modules, {len(jobs)} runs, {len(failed)} failed ({len(failed) - len(new)} known)")
    for entry in sorted(set(failed) & expected):
        print(f"  known: {entry}")
    for entry in new:
        print(f"  NEW FAILURE: {entry}: {failed[entry]}")
    for entry in fixed:
        print(f"  FIXED (remove from known-failures.txt): {entry}")
    return 1 if new or fixed else 0


if __name__ == "__main__":
    sys.exit(main())
