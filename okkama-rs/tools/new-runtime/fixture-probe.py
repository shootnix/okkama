#!/usr/bin/env python3
"""Recompile focused async fixtures and record origin/cold/JIT observations.

This is a differential fixture probe, not full conformance or a benchmark.
Known differences remain in the output; they are never relabelled as parity.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
from probe_process import run_bounded, ResourceLimitExceeded, positive_int
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[3]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--compiler', type=Path, required=True)
    parser.add_argument('--origin', type=Path, required=True)
    parser.add_argument('--runtime', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    compiler, origin, runtime = (path.resolve() for path in (args.compiler, args.origin, args.runtime))
    known = {'async-worker-framing': 'Origin locates the final OKW1 byte sequence inside returned string data and corrupts the response; the new engine uses a length trailer.', 'async-group-cancel-cleanup': 'Origin discards a real cleanup failure after explicit group cancellation; new engine retains it.'}
    records = []
    with tempfile.TemporaryDirectory(prefix='sparrow-fixture-probe-') as temporary:
        for source in sorted((ROOT/'okkama-rs/crates/sparrow-engine/tests/fixtures').glob('async-*.ok')):
            binary = Path(temporary)/(source.stem+'.spb')
            record = {'source': str(source.relative_to(ROOT)), 'source_sha256': hashlib.sha256(source.read_bytes()).hexdigest(), 'runs': []}
            try:
                built = run_bounded([str(compiler), 'build', str(source), '--target', 'sparrow', '-o', str(binary)], cwd=ROOT, capture_output=True, timeout=30)
            except (subprocess.TimeoutExpired, ResourceLimitExceeded) as error:
                record.update(status=getattr(error, 'reason', 'COMPILE_TIMEOUT'))
                records.append(record)
                print(source.stem, record['status'], flush=True)
                continue
            if built.returncode:
                record.update(status='COMPILE_FAILURE', stderr=built.stderr.decode('utf-8', 'replace'))
            else:
                record['spb_sha256'] = hashlib.sha256(binary.read_bytes()).hexdigest()
                for mode, executable, flags in [('origin', origin, []), ('cold', runtime, ['--cold']), ('jit', runtime, [])]:
                    try:
                        ran = run_bounded([str(executable), *flags, str(binary)], cwd=ROOT, capture_output=True, timeout=20, env=dict(os.environ, OKKAMA_TRACE='off'))
                        record['runs'].append({'mode': mode, 'exit': ran.returncode, 'stdout': ran.stdout.decode('utf-8', 'replace'), 'stderr': ran.stderr.decode('utf-8', 'replace')})
                    except (subprocess.TimeoutExpired, ResourceLimitExceeded) as error:
                        record['runs'].append({'mode': mode, 'failure': getattr(error, 'reason', 'TIMEOUT')})
                def normalized(run):
                    return run.get('exit'), run.get('stdout'), run.get('stderr', '').removeprefix('sparrow-next: '), run.get('failure')
                baseline = normalized(record['runs'][0])
                parity = all(normalized(run) == baseline for run in record['runs'][1:])
                record['status'] = 'PROBE_FAILURE' if any('failure' in run for run in record['runs']) else 'MATCH' if parity else 'KNOWN_DIFFERENCE' if source.stem in known else 'MISMATCH'
                if source.stem in known:
                    record['known_difference'] = known[source.stem]
            records.append(record)
            print(source.stem, record['status'], flush=True)
    args.output.write_text(json.dumps({'scope': 'focused compiler async fixtures; not full conformance', 'compiler_revision': '1c289f8b3307858470e97489043cb4486eefa8ea', 'compiler_sha256': hashlib.sha256(compiler.read_bytes()).hexdigest(), 'origin_sha256': hashlib.sha256(origin.read_bytes()).hexdigest(), 'runtime_sha256': hashlib.sha256(runtime.read_bytes()).hexdigest(), 'fixtures': records}, indent=2)+'\n')
    if any(record['status'] not in {'MATCH', 'KNOWN_DIFFERENCE'} for record in records):
        raise SystemExit(1)


if __name__ == '__main__':
    main()
