#!/usr/bin/env python3
"""Compare the Sparrow core (`sparrow`) with the previous VM (`sparrow-origin`)
on conformance programs.

Each selected run or test case is built with an explicit compiler and stdlib
(and the conformance harness when the case asks for it), then run under
`caged.py` on both binaries, sequentially. A case passes when the core's
stdout and exit status equal the previous VM's and, for failures, the first
line of the error report is the same. A case where only the core matches the
case's own expectation (stdout and exit status in case.json; for example std
modules that need natives the previous VM lacks) passes as `spec`. Program
arguments, stdin and environment variables of a case are passed through;
cases needing networks, workers or process spawning are skipped (the cage
forbids them).
"""
import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
CAGE = HERE / 'caged.py'


def run(binary, spb, cwd, env, spec=None, test_flag='--test'):
    spec = spec or {}
    flags = [test_flag] if spec.get('mode') == 'test' else []
    command = [sys.executable, str(CAGE), '--cpu-seconds', '30', '--timeout', '60',
               '--writable', cwd, '--', str(binary), *flags, str(spb), *spec.get('args', [])]
    stdin = (Path(cwd) / 'case' / spec['stdin']).read_bytes() if spec.get('stdin') else None
    env = dict(env, **spec.get('env', {}))
    # Each engine gets its own copy of the case directory: programs write files.
    own = Path(tempfile.mkdtemp(dir=cwd))
    shutil.copytree(Path(cwd) / 'case', own / 'case')
    command[command.index('--writable') + 1] = own
    result = subprocess.run(command, cwd=own, env=env, input=stdin, capture_output=True)
    return result.returncode, result.stdout, result.stderr.decode(errors='replace')


RUNNABLE = {'sqlite', 'filesystem', 'env-vars', 'unicode-data', 'args', 'stdin', 'real-clock',
            'real-random', 'stderr'}


def message(stderr):
    lines = [l for l in stderr.splitlines() if l.strip() and not l.startswith('caged:')]
    if not lines:
        return ''
    first = lines[0]
    for prefix in ('sparrow: ', 'sparrow-origin: '):
        if first.startswith(prefix):
            first = first[len(prefix):]
    return first


def matches_spec(spec, result):
    """Does a run satisfy the case's own expected exit status and stdout?"""
    expect = spec.get('expect', {})
    want = expect.get('stdout')
    if isinstance(want, dict):
        want = want.get('exact', want.get('text'))
    if want is None or result[0] != expect.get('exit_status', 0):
        return False
    return result[1].decode(errors='replace') == want


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--compiler', default='okkama')
    p.add_argument('--stdlib', type=Path, required=True)
    p.add_argument('--core-stdlib', type=Path,
                   help='stdlib for the core build (default: --stdlib); '
                        'for std changes that need the core')
    p.add_argument('--origin', type=Path, required=True, help='sparrow-origin binary')
    p.add_argument('--core', type=Path, required=True, help='sparrow binary')
    p.add_argument('--work', type=Path, required=True)
    p.add_argument('levels', nargs='+', help='level directory prefixes, e.g. L01 L04')
    a = p.parse_args()
    env = dict(os.environ, OKKAMA_STDLIB_DIR=str(a.stdlib))
    a.work.mkdir(parents=True, exist_ok=True)
    totals = {'pass': 0, 'spec': 0, 'fail': 0, 'build': 0, 'skip': 0}
    for level in sorted((ROOT / 'conformance/cases').iterdir()):
        if not any(level.name.startswith(prefix) for prefix in a.levels):
            continue
        for case in sorted(level.iterdir()):
            spec_path = case / 'case.json'
            if not spec_path.exists():
                continue
            spec = json.loads(spec_path.read_text())
            needs = set(spec.get('requires', {}).get('environment', [])) - {'stdout'}
            if spec.get('mode') not in ('run', 'test') or needs - RUNNABLE:
                totals['skip'] += 1
                continue
            work = Path(tempfile.mkdtemp(dir=a.work))
            shutil.copytree(case, work / 'case')
            if spec.get('harness'):
                shutil.copytree(ROOT / 'conformance/harness/okkama/conformance', work / 'case/conformance')
            spb = work / 'main.spb'
            # Test-mode cases build their test runner (`--tests`).
            tests = ['--tests'] if spec.get('mode') == 'test' else []
            built = subprocess.run([a.compiler, 'build', *tests, str(work / 'case' / spec['entry']),
                                    '--target', 'sparrow', '-o', str(spb)], env=env, capture_output=True)
            if built.returncode:
                totals['build'] += 1
                continue
            core_spb = spb
            if a.core_stdlib:
                core_spb = work / 'core.spb'
                core_env = dict(env, OKKAMA_STDLIB_DIR=str(a.core_stdlib))
                rebuilt = subprocess.run([a.compiler, 'build', *tests, str(work / 'case' / spec['entry']),
                                          '--target', 'sparrow', '-o', str(core_spb)], env=core_env,
                                         capture_output=True)
                if rebuilt.returncode:
                    totals['fail'] += 1
                    print(f"FAIL {spec['id']}: core stdlib build: {rebuilt.stderr.decode(errors='replace')[:160]!r}", flush=True)
                    continue
            origin = run(a.origin, spb, work, env, spec, test_flag='test')
            core = run(a.core, core_spb, work, env, spec)
            ok = core[0] == origin[0] and core[1] == origin[1]
            detail = ''
            if ok and core[0] != 0 and spec.get('mode') != 'test' and message(origin[2]) != message(core[2]):
                ok = False
                detail = f' text: origin={message(origin[2])[:80]!r} core={message(core[2])[:80]!r}'
            elif not ok:
                detail = (f' exit {origin[0]}/{core[0]} stdout same={core[1] == origin[1]}'
                          f' core: {message(core[2])[:90]!r}')
            if not ok and matches_spec(spec, core):
                totals['spec'] += 1
                print(f"SPEC {spec['id']}: core matches case.json;{detail}", flush=True)
            else:
                totals['pass' if ok else 'fail'] += 1
                if not ok:
                    print(f"FAIL {spec['id']}:{detail}", flush=True)
            shutil.rmtree(work, ignore_errors=True)
    print(' '.join(f'{k}={v}' for k, v in totals.items()))


if __name__ == '__main__':
    main()
