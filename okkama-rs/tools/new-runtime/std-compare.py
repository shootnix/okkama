#!/usr/bin/env python3
"""Run the embedded tests of every stdlib module on the Sparrow core (`sparrow`)
and compare them with the previous VM (`sparrow-origin`).

Each module is built with its test runner (`okkama build --tests`) against the
given stdlib, then run in test mode under `caged.py` on both, in its
own copy of a work directory. A module passes when the exit status and the
test report are identical. Modules without tests or that do not build are
counted apart; reports that mention an unsupported construct of the core are
listed as unsupported; natives missing from both engines are counted apart,
as are modules whose tests pass only on the core.
"""
import argparse
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
CAGE = HERE / 'caged.py'


def run(binary, spb, cwd, env, test_flag='--test'):
    own = Path(tempfile.mkdtemp(dir=cwd))
    command = [sys.executable, str(CAGE), '--cpu-seconds', '60', '--timeout', '120',
               '--writable', str(own), '--', str(binary), test_flag, str(spb)]
    result = subprocess.run(command, cwd=own, env=env, capture_output=True)
    return result.returncode, result.stdout, result.stderr.decode(errors='replace')


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--compiler', default='okkama')
    p.add_argument('--stdlib', type=Path, required=True)
    p.add_argument('--reference', type=Path, required=True, help='sparrow-origin binary')
    p.add_argument('--core', type=Path, required=True)
    p.add_argument('--work', type=Path, required=True)
    p.add_argument('prefixes', nargs='*', help='module path prefixes, e.g. json db/')
    a = p.parse_args()
    env = dict(os.environ, OKKAMA_STDLIB_DIR=str(a.stdlib))
    a.work.mkdir(parents=True, exist_ok=True)
    totals = {'pass': 0, 'fail': 0, 'core-only-pass': 0, 'unsupported': 0, 'both-unsupported': 0,
              'build': 0, 'no-tests': 0}
    for source in sorted(a.stdlib.rglob('*.ok')):
        module = source.relative_to(a.stdlib).as_posix()
        if a.prefixes and not any(module.startswith(prefix) for prefix in a.prefixes):
            continue
        if 'test "' not in source.read_text(errors='replace'):
            totals['no-tests'] += 1
            continue
        work = Path(tempfile.mkdtemp(dir=a.work))
        spb = work / 'tests.spb'
        built = subprocess.run([a.compiler, 'build', '--tests', str(source), '--target', 'sparrow',
                                '-o', str(spb)], env=env, capture_output=True)
        if built.returncode:
            totals['build'] += 1
            shutil.rmtree(work, ignore_errors=True)
            continue
        reference = run(a.reference, spb, work, env, test_flag='test')
        core = run(a.core, spb, work, env)
        missing_core = b'is not available in sparrow-core' in core[1] or 'sparrow-core yet' in core[2]
        missing_reference = b'in new engine' in reference[1]
        if missing_core and missing_reference:
            totals['both-unsupported'] += 1
        elif 'sparrow-core yet' in core[2] or 'not supported by sparrow-core' in core[2]:
            totals['unsupported'] += 1
            print(f'UNSUPPORTED {module}: {core[2].strip().splitlines()[-1][:110]}', flush=True)
        elif core[0] == reference[0] and core[1] == reference[1]:
            totals['pass'] += 1
        elif core[0] == 0 and reference[0] != 0:
            totals['core-only-pass'] += 1
            print(f'CORE-ONLY-PASS {module}', flush=True)
        else:
            totals['fail'] += 1
            ref_lines = reference[1].decode(errors='replace').splitlines()
            core_lines = core[1].decode(errors='replace').splitlines()
            diff = next((f'{r!r} / {c!r}' for r, c in zip(ref_lines, core_lines) if r != c),
                        f'lines {len(ref_lines)}/{len(core_lines)}')
            print(f'FAIL {module}: exit {reference[0]}/{core[0]} {diff[:160]} '
                  f'core stderr: {core[2].strip()[-120:]!r}', flush=True)
        shutil.rmtree(work, ignore_errors=True)
    print(' '.join(f'{k}={v}' for k, v in totals.items()))


if __name__ == '__main__':
    main()
