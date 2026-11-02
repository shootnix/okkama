"""Count region traversal work; timings from this instrumented build are not benchmarks."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
FIELDS = ['register', 'transfer', 'keep_roots', 'collect_calls', 'values_popped',
          'object_encounters', 'owner_rejected', 'duplicate_rejected', 'accepted_objects']
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--compiler', required=True, type=Path)
parser.add_argument('--out', required=True, type=Path)
args = parser.parse_args()
env = dict(os.environ, OKKAMA_STDLIB_DIR=str(ROOT/'std'), OKKAMA_SQLITE_INLINE_BENCH='1',
           DB_BENCH_ROWS='5000', DB_BENCH_WARMUPS='4')
for key in ('OKKAMA_JIT', 'OKKAMA_JIT_STATS', 'OKKAMA_JIT_THRESHOLD', 'OKKAMA_GC'):
    env.pop(key, None)
probe = subprocess.run([str(args.compiler), 'run', '--engine', 'sparrow', str(HERE/'inline-probe.ok')],
                       env=env, text=True, capture_output=True, check=True)
assert probe.stdout.strip() == 'inline', probe.stdout
result = dict(fields=FIELDS, method='Two repetitions; identical setup and four warmups of 500 lookups. Differences across 500/1000/1500 measured lookups remove fixed costs. Counters cover whole main-thread lifetime. Accepted objects are visits, not unique allocations.',
              compiler_sha256=hashlib.sha256(args.compiler.read_bytes()).hexdigest(),
              workload_sha256=hashlib.sha256((HERE/'prepared-profile.ok').read_bytes()).hexdigest(), samples=[])
for repetition in range(2):
    for count in (500, 1000, 1500):
        run = subprocess.run([str(args.compiler), 'run', '--engine', 'sparrow', str(HERE/'prepared-profile.ok')],
                             env=dict(env, DB_BENCH_LOOKUPS=str(count)), text=True, capture_output=True, check=True)
        lines = run.stdout.splitlines()
        assert lines[0] == 'PROFILE_READY'
        phase, nanos, checksum = lines[1].split()
        expected = sum(i + len('user-'+str(i)) + (5 if i%3 == 0 else 0 if i%5 == 0 else 4) for i in range(count))
        assert phase == 'lookup_prepared' and int(checksum) == expected
        counters = re.findall(r'region_walk_stats (\[[^\n]+\])', run.stderr)
        assert len(counters) == 1, run.stderr
        values = json.loads(counters[0])
        assert len(values) == len(FIELDS)
        assert sum(values[:3]) == values[3]
        assert sum(values[6:]) == values[5]
        sample = dict(repetition=repetition, lookups=count, checksum=expected, counters=dict(zip(FIELDS, values)))
        result['samples'].append(sample)
        print(json.dumps(sample), flush=True)
result['deltas_per_query'] = []
for repetition in range(2):
    samples = [s for s in result['samples'] if s['repetition'] == repetition]
    for before, after in zip(samples, samples[1:]):
        result['deltas_per_query'].append(dict(repetition=repetition, start=before['lookups'], end=after['lookups'],
            counters={key:(after['counters'][key]-before['counters'][key])/500 for key in FIELDS}))
args.out.write_text(json.dumps(result, indent=2)+'\n')
print(json.dumps(result['deltas_per_query'], indent=2))
