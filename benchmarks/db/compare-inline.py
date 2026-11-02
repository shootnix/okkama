#!/usr/bin/env python3
"""Isolate SQLite worker delivery; cache0 requests zero, which Python 3.9 clamps to five.
Use python-cache-probe.py to verify actual SQL preparation behavior.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import sqlite3
import statistics
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', type=Path, required=True,
                        help='release executable built with sqlite-inline-bench')
    parser.add_argument('--runs', type=int, default=5)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if args.runs < 1:
        parser.error('runs must be positive')
    compiler = args.compiler.resolve()
    env = dict(os.environ, OKKAMA_STDLIB_DIR=str(ROOT/'std'),
               DB_BENCH_ROWS='5000', DB_BENCH_LOOKUPS='500', DB_BENCH_WARMUPS='4')
    for key in ('OKKAMA_JIT', 'OKKAMA_ENGINE', 'OKKAMA_GC', 'OKKAMA_JIT_THRESHOLD',
                'OKKAMA_SQLITE_INLINE_BENCH', 'DB_BENCH_SQLITE_CACHE'):
        env.pop(key, None)
    # Fail rather than silently benchmarking a production executable twice.
    for engine in ('ir', 'sparrow'):
        for inline in (False, True):
            probe_env = dict(env, OKKAMA_SQLITE_INLINE_BENCH=str(int(inline)))
            probe = subprocess.run([str(compiler), 'run', '--engine', engine,
                                    str(HERE/'inline-probe.ok')], cwd=ROOT, env=probe_env,
                                   capture_output=True, text=True, timeout=60)
            if probe.returncode or probe.stdout.strip() != ('inline' if inline else 'worker'):
                raise RuntimeError(probe.stdout + probe.stderr)
    def row(i):
        return i + len('user-'+str(i)) + (5 if i % 3 == 0 else 0 if i % 5 == 0 else 4)
    lookup_sum = sum(row(i) for i in range(500))
    expected = dict(projection_only=5000*row(0), scan_raw=15000,
                    scan_stream=sum(map(row, range(5000))), scan_all=sum(map(row, range(5000))),
                    lookup_prepared=lookup_sum, lookup_statement=lookup_sum,
                    insert_transaction=500*499//2)
    cache_checks = {}
    for cache in (None, 0):
        options = {} if cache is None else {'cached_statements': cache}
        with sqlite3.connect(':memory:', **options) as db:
            db.execute('CREATE TABLE t(n INTEGER)')
            db.execute('INSERT INTO t VALUES (1)')
            before = db.execute('SELECT n FROM t').fetchone()[0]
            db.execute('UPDATE t SET n=2')
            after = db.execute('SELECT n FROM t').fetchone()[0]
            assert (before, after) == (1, 2)
            cache_checks['default' if cache is None else str(cache)] = [before, after]
        db.close()
    result = dict(method='Sequential rotating modes; same feature-enabled compiler for worker and inline. Native Future is already complete in inline mode. No claim about cancellation equivalence.',
                  config=dict(runs=args.runs, warmups=4, rows=5000, lookups=500),
                  environment=dict(platform=platform.platform(), python=sys.version,
                                   python_sqlite=sqlite3.sqlite_version, okkama_sqlite='3.50.2',
                                   compiler=str(compiler), compiler_sha256=digest(compiler)),
                  sources={p.name: digest(p) for p in (HERE/'main.ok', HERE/'main.py', HERE/'inline-probe.ok', Path(__file__))},
                  stdlib_hashes={str(p.relative_to(ROOT/'std')): digest(p) for p in sorted((ROOT/'std').rglob('*.ok'))},
                  sqlite_provider_sha256=digest(ROOT/'okkama-rs/crates/okkama-runtime-core/src/native/sqlite.rs'),
                  expected=expected, same_select_after_update=cache_checks, samples=[])
    modes = [('python', 'cache_default'), ('python', 'cache0'), ('ir', 'worker'),
             ('ir', 'inline'), ('sparrow', 'worker'), ('sparrow', 'inline')]
    args.out.parent.mkdir(parents=True, exist_ok=True)
    for i in range(args.runs):
        order = modes[i % len(modes):]+modes[:i % len(modes)]
        if i % 2:
            order = order[::-1]
        for engine, mode in order:
            run_env = dict(env, OKKAMA_SQLITE_INLINE_BENCH='1' if mode == 'inline' else '0')
            if mode == 'cache0':
                run_env['DB_BENCH_SQLITE_CACHE'] = '0'
            command = ([sys.executable, str(HERE/'main.py')] if engine == 'python' else
                       [str(compiler), 'run', '--engine', engine, str(HERE/'main.ok')])
            process = subprocess.run(command, cwd=ROOT, env=run_env, text=True,
                                     capture_output=True, timeout=180)
            if process.returncode:
                raise RuntimeError(process.stdout + process.stderr)
            phases = {}
            for line in process.stdout.splitlines():
                phase, ns, checksum = line.split()
                assert int(checksum) == expected[phase], (engine, mode, phase, checksum)
                phases[phase] = int(ns)/1e6
            assert phases.keys() == expected.keys()
            result['samples'].append(dict(iteration=i, engine=engine, mode=mode, ms=phases))
            args.out.write_text(json.dumps(result, indent=2)+'\n')
            print(i, engine, mode, phases, flush=True)
    result['summary'] = {engine+'_'+mode: {phase: statistics.median(
        s['ms'][phase] for s in result['samples'] if s['engine'] == engine and s['mode'] == mode)
        for phase in expected} for engine, mode in modes}
    args.out.write_text(json.dumps(result, indent=2)+'\n')
    print(json.dumps(result['summary'], indent=2))


if __name__ == '__main__':
    main()
