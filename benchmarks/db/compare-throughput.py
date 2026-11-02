#!/usr/bin/env python3
"""Compare stdlib snapshots and Python using identical warmed SQLite workloads."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--before-stdlib', type=Path, required=True)
    parser.add_argument('--after-stdlib', type=Path, default=ROOT/'std')
    parser.add_argument('--compiler', type=Path, default=ROOT/'okkama-rs/target/release/okkama')
    parser.add_argument('--runs', type=int, default=5)
    parser.add_argument('--warmups', type=int, default=4)
    parser.add_argument('--rows', type=int, default=5000)
    parser.add_argument('--lookups', type=int, default=500)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if min(args.runs, args.warmups, args.rows, args.lookups) < 1:
        parser.error('counts must be positive')
    roots = {'before': args.before_stdlib.resolve(), 'after': args.after_stdlib.resolve()}
    env = dict(os.environ, DB_BENCH_ROWS=str(args.rows), DB_BENCH_LOOKUPS=str(args.lookups),
               DB_BENCH_WARMUPS=str(args.warmups))
    for key in ('OKKAMA_JIT', 'OKKAMA_ENGINE', 'OKKAMA_GC', 'OKKAMA_JIT_THRESHOLD'):
        env.pop(key, None)
    def row(i):
        return i + len('user-'+str(i)) + (5 if i % 3 == 0 else 0 if i % 5 == 0 else 4)
    lookup_sum = sum(row(i % args.rows) for i in range(args.lookups))
    expected = dict(projection_only=args.rows*row(0), scan_raw=args.rows*3,
                    scan_stream=sum(map(row, range(args.rows))), scan_all=sum(map(row, range(args.rows))),
                    lookup_prepared=lookup_sum, lookup_statement=lookup_sum,
                    insert_transaction=args.lookups*(args.lookups-1)//2)
    result = {'method': 'Sequential paired processes, rotating engine and before/after order; Python measured in each round; default JIT settings',
              'config': {k: getattr(args, k) for k in ('runs', 'warmups', 'rows', 'lookups')},
              'environment': {'platform': platform.platform(), 'python': sys.version,
                              'python_sqlite': subprocess.check_output([sys.executable, '-c', 'import sqlite3; print(sqlite3.sqlite_version)'], text=True).strip(),
                              'compiler_sha256': digest(args.compiler)},
              'sources': {p.name: digest(p) for p in (HERE/'main.ok', HERE/'main.py', Path(__file__))},
              'stdlib_roots': {k: str(v) for k, v in roots.items()},
              'stdlib_hashes': {k: {str(p.relative_to(v)): digest(p) for p in sorted(v.rglob('*.ok'))} for k, v in roots.items()},
              'expected': expected, 'samples': []}
    args.out.parent.mkdir(parents=True, exist_ok=True)
    def run(engine, version, iteration):
        if engine == 'python':
            command = [sys.executable, str(HERE/'main.py')]
        else:
            env['OKKAMA_STDLIB_DIR'] = str(roots[version])
            command = [str(args.compiler.resolve()), 'run', '--engine', engine, str(HERE/'main.ok')]
        process = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True, timeout=240)
        if process.returncode:
            raise RuntimeError(process.stdout + process.stderr)
        phases = {}
        for line in process.stdout.splitlines():
            name, ns, checksum = line.split()
            if int(checksum) != expected[name]:
                raise AssertionError((engine, version, name, checksum, expected[name]))
            phases[name] = int(ns)/1e6
        if phases.keys() != expected.keys():
            raise AssertionError('missing phases')
        result['samples'].append(dict(iteration=iteration, engine=engine, version=version, ms=phases))
        args.out.write_text(json.dumps(result, indent=2)+'\n')
        print(iteration, engine, version, phases, flush=True)
    for i in range(args.runs):
        engines = ['ir', 'sparrow', 'python']
        engines = engines[i % 3:] + engines[:i % 3]
        for engine in engines:
            for version in (['reference'] if engine == 'python' else ['before', 'after'] if i % 2 == 0 else ['after', 'before']):
                run(engine, version, i)
    result['summary'] = {engine: {phase: {version: statistics.median(
        s['ms'][phase] for s in result['samples'] if s['engine'] == engine and s['version'] == version)
        for version in (['reference'] if engine == 'python' else roots)} for phase in expected}
        for engine in ('ir', 'sparrow', 'python')}
    args.out.write_text(json.dumps(result, indent=2)+'\n')
    print(json.dumps(result['summary'], indent=2))

if __name__ == '__main__':
    main()
