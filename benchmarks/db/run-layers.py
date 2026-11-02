#!/usr/bin/env python3
"""Sequential same-SQL comparison of native, driver and Query overhead."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess

ROOT = Path(__file__).resolve().parents[2]

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--runs', type=int, default=5)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if args.runs < 1:
        parser.error('runs must be positive')
    source = ROOT/'benchmarks/db/layers.ok'
    compiler = ROOT/'okkama-rs/target/release/okkama'
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    expected = dict(sync=500, async_=500, native_one=3500, driver_one=3500, query_one=3500)
    expected['async'] = expected.pop('async_')
    result = dict(iterations=500, warmups_per_phase=4, samples_per_engine=args.runs,
                  source_sha256=digest(source), compiler_sha256=digest(compiler), samples=[])
    env = dict(os.environ, OKKAMA_STDLIB_DIR=str(ROOT/'std'))
    for key in ['OKKAMA_ENGINE', 'OKKAMA_JIT', 'OKKAMA_JIT_THRESHOLD', 'OKKAMA_GC']:
        env.pop(key, None)
    for i in range(args.runs):
        for engine in (['ir', 'sparrow'] if i % 2 == 0 else ['sparrow', 'ir']):
            run = subprocess.run([str(compiler), 'run', '--engine', engine, str(source)],
                                 env=env, text=True, capture_output=True, timeout=120)
            if run.returncode:
                raise RuntimeError(run.stdout + run.stderr)
            phases = {}
            for line in run.stdout.splitlines():
                name, ns, total = line.split()
                if int(total) != expected[name]:
                    raise AssertionError((name, total))
                phases[name] = int(ns)/1e6
            if phases.keys() != expected.keys():
                raise AssertionError('missing phases')
            result['samples'].append(dict(iteration=i, engine=engine, ms=phases))
            print(engine, i, phases, flush=True)
    result['summary'] = {e: {k: statistics.median(s['ms'][k] for s in result['samples']
                                                 if s['engine'] == e)
                             for k in expected} for e in ['ir', 'sparrow']}
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, indent=2)+'\n')

if __name__ == '__main__':
    main()
