#!/usr/bin/env python3
"""Run stdlib tests with an explicitly selected engine binary; retain failures."""
import argparse
import hashlib
import json
import os
from pathlib import Path
from probe_process import run_bounded, ResourceLimitExceeded, positive_int, map_bounded
import re
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[3]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', type=Path, required=True)
parser.add_argument('--out', type=Path, required=True)
parser.add_argument('--jobs', type=positive_int, default=1)
args = parser.parse_args()
binary = args.binary.resolve()
paths = [p for p in sorted((ROOT / 'std').rglob('*.ok'))
         if re.search(r'^\s*(?:async\s+)?test\s+"', p.read_text(), re.M)]


def run(path):
    command = [str(binary), 'run', '--engine', 'sparrow', '-t', str(path)]
    # Relative output files cannot overwrite source or interfere with other tests.
    with tempfile.TemporaryDirectory(prefix='nom-std-') as work:
        env = dict(os.environ, OKKAMA_TRACE='off',
                   OKKAMA_STDLIB_DIR=str(ROOT / 'std'), TMPDIR=work)
        try:
            result = run_bounded(command, cwd=work, env=env,
                                    stdin=subprocess.DEVNULL, capture_output=True,
                                    timeout=60)
            code, stdout, stderr = result.returncode, result.stdout, result.stderr
        except (subprocess.TimeoutExpired, ResourceLimitExceeded) as error:
            code, stdout, stderr = getattr(error, 'reason', 'timeout'), error.stdout or b'', error.stderr or b''
    return dict(path=str(path.relative_to(ROOT)),
                sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                code=code, stdout=stdout.decode('utf-8', 'replace'),
                stderr=stderr.decode('utf-8', 'replace'))


results = []
for result in map_bounded(run, paths, args.jobs):
    results.append(result)
    print(f"[{len(results)}/{len(paths)}] {result['code']} {result['path']}", flush=True)
report = dict(binary=str(binary), binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
              icu_libdir=os.environ.get('OKKAMA_ICU_LIBDIR'),
              total=len(results), passed=sum(r['code'] == 0 for r in results), results=results)
args.out.parent.mkdir(parents=True, exist_ok=True)
args.out.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
raise SystemExit(0 if report['passed'] == report['total'] else 1)
