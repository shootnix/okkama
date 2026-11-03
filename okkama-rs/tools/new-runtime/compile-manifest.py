#!/usr/bin/env python3
"""Compile/check every non-OCS source; record diagnostics without assuming success."""
import argparse
import hashlib
import json
import os
from pathlib import Path
from probe_process import run_bounded, ResourceLimitExceeded, positive_int, map_bounded
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[3]
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--binary', type=Path, required=True)
p.add_argument('--out', type=Path, required=True)
p.add_argument('--jobs', type=positive_int, default=1)
a = p.parse_args()
binary = a.binary.resolve()
manifest = json.loads((ROOT / 'okkama-rs/docs/new-runtime/inventory.json').read_text())
sources = [s for s in manifest['programs'] if s['category'] != 'conformance']
catalog = json.loads((ROOT / 'examples/gallery/catalog.json').read_text())
rejects = {'examples/gallery/' + row['entry']: row for row in catalog['examples'] if row['mode'] == 'reject'}
parser_fixture = 'okkama-rs/crates/okkama-compiler/tests/fixtures/all-syntax.ok'
historical = 'benchmarks/reports/2026-10-31-v0.3.1/json/orders.ok'


def check(source):
    path = ROOT / source['path']
    modes = (['main'] if source['main'] else []) + (['tests'] if source['tests'] else [])
    modes = modes or ['check']
    if source['path'] == parser_fixture:
        modes = ['parse']
    results = []
    for mode in modes:
        with tempfile.TemporaryDirectory(prefix='nom-compile-') as work:
            command = [str(binary)]
            if mode == 'parse':
                command += ['ast', str(path)]
            elif mode == 'check':
                command += ['check', str(path)]
            else:
                command += ['build', str(path), '--target', 'sparrow', '-o', work + '/program.spb']
                if mode == 'tests':
                    command += ['-t']
            try:
                result = run_bounded(command, cwd=path.parent,
                                        env=dict(os.environ, OKKAMA_STDLIB_DIR=str(ROOT / 'std')),
                                        stdin=subprocess.DEVNULL, capture_output=True, timeout=60)
                code, stdout, stderr = result.returncode, result.stdout, result.stderr
            except (subprocess.TimeoutExpired, ResourceLimitExceeded) as error:
                code, stdout, stderr = getattr(error, 'reason', 'timeout'), error.stdout or b'', error.stderr or b''
        results.append(dict(mode=mode, code=code, stdout=stdout.decode('utf-8', 'replace'),
                            stderr=stderr.decode('utf-8', 'replace')))
    expectation = 'review_required' if any(r['code'] != 0 for r in results) else 'accepted'
    basis = None
    if source['path'] in rejects:
        # The catalog's diagnostic wording belongs to the Perl analyzer. Preserve
        # Rust diagnostics separately; a timeout/signal is never a valid rejection.
        expectation = 'expected_rejection' if all(r['code'] == 1 and 'semantic error:' in r['stderr'] for r in results) else 'unexpected_rejection_result'
        basis = dict(catalog='examples/gallery/catalog.json', case=rejects[source['path']])
    elif source['path'] == parser_fixture:
        basis = 'Parser/printer fixture, referenced by tests/parser_robustness.rs and tests/printer.rs; not a complete executable module.'
    elif source['path'] == historical:
        expectation = 'historical_source_incompatible' if any(r['code'] != 0 for r in results) else 'accepted'
        basis = 'Immutable evidence from the v0.3.1 benchmark report; imports the former json/parse API. Not an executable current-version example.'
    return dict(path=source['path'], sha256=source['sha256'], results=results,
                runtime='NOT_RUN', expectation=expectation, expectation_basis=basis)


results = []
for result in map_bounded(check, sources, a.jobs):
    results.append(result)
    print(f"[{len(results)}/{len(sources)}] {result['expectation']} {result['path']}", flush=True)
a.out.parent.mkdir(parents=True, exist_ok=True)
a.out.write_text(json.dumps(dict(binary=str(binary),
                               binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                               results=results), ensure_ascii=False, indent=2) + '\n')
