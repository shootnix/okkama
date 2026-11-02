"""Measure Rust allocation request deltas with the existing mimalloc backend."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
FIELDS = ['alloc_calls', 'alloc_requested_bytes', 'realloc_calls', 'realloc_requested_bytes', 'dealloc_calls']
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--compiler', required=True, type=Path)
p.add_argument('--out', required=True, type=Path)
p.add_argument('--require-scopes', action='store_true')
args = p.parse_args()
env = dict(os.environ, OKKAMA_STDLIB_DIR=str(ROOT/'std'), OKKAMA_SQLITE_INLINE_BENCH='1',
           DB_BENCH_ROWS='5000', DB_BENCH_WARMUPS='4')
for key in ('OKKAMA_JIT', 'OKKAMA_JIT_STATS', 'OKKAMA_JIT_THRESHOLD', 'OKKAMA_GC'):
    env.pop(key, None)
probe = subprocess.run([str(args.compiler), 'run', '--engine', 'sparrow', str(HERE/'inline-probe.ok')],
                       env=env, text=True, capture_output=True, check=True)
assert probe.stdout.strip() == 'inline', probe.stdout
result = dict(method='GlobalAlloc requests forwarded to unchanged MiMalloc. Two repetitions with fixed setup and four warmups of 500 queries. Differences remove fixed costs. C allocations, allocator internals and RSS are excluded; realloc bytes are full new requested sizes, not copied bytes.',
              compiler_sha256=hashlib.sha256(args.compiler.read_bytes()).hexdigest(), samples=[])
for repetition in range(2):
    for count in (500, 1000, 1500):
        run = subprocess.run([str(args.compiler), 'run', '--engine', 'sparrow', str(HERE/'prepared-profile.ok')],
                             env=dict(env, DB_BENCH_LOOKUPS=str(count)), text=True, capture_output=True, check=True)
        phase, nanos, checksum = run.stdout.splitlines()[1].split()
        expected = sum(i + len('user-'+str(i)) + (5 if i%3 == 0 else 0 if i%5 == 0 else 4) for i in range(count))
        assert phase == 'lookup_prepared' and int(checksum) == expected
        found = re.findall(r'allocation_stats (\[[^\n]+\])', run.stderr)
        assert len(found) == 1, run.stderr
        values = json.loads(found[0])
        assert len(values) == len(FIELDS)
        sample = dict(repetition=repetition, lookups=count, checksum=expected, counters=dict(zip(FIELDS, values)))
        scopes = re.findall(r'allocation_scopes (\[[^\n]+\])', run.stderr)
        if args.require_scopes:
            assert len(scopes) == 1, run.stderr
        if scopes:
            buckets = json.loads(scopes[0])
            assert len(buckets) == 3 and all(len(row) == len(FIELDS) for row in buckets)
            assert [sum(row[col] for row in buckets) for col in range(len(FIELDS))] == values
            sample['scopes'] = {name:dict(zip(FIELDS, row)) for name,row in zip(('rest','region_outside_collect','collect'), buckets)}
            if args.require_scopes:
                assert sample['scopes']['collect']['alloc_calls'] > 0
        result['samples'].append(sample)
        print(json.dumps(sample), flush=True)
result['deltas_per_query'] = []
for repetition in range(2):
    samples = [s for s in result['samples'] if s['repetition'] == repetition]
    for before, after in zip(samples, samples[1:]):
        result['deltas_per_query'].append(dict(repetition=repetition, start=before['lookups'], end=after['lookups'],
            counters={key:(after['counters'][key]-before['counters'][key])/500 for key in FIELDS}))
        if 'scopes' in before:
            result['deltas_per_query'][-1]['scopes'] = {
                name:{key:(after['scopes'][name][key]-before['scopes'][name][key])/500 for key in FIELDS}
                for name in before['scopes']}
result['sources_sha256'] = {str(f.relative_to(ROOT)):hashlib.sha256(f.read_bytes()).hexdigest() for f in
    (Path(__file__), HERE/'prepared-profile.ok', HERE/'allocation-counter/src/lib.rs', ROOT/'okkama-rs/crates/okkama-cli/src/main.rs',
     ROOT/'okkama-rs/crates/okkama-runtime-core/src/heap.rs')}
args.out.write_text(json.dumps(result, indent=2)+'\n')
print(json.dumps(result['deltas_per_query'], indent=2))
