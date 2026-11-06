#!/usr/bin/env python3
"""Compare sparrow-core with the reference engines on conformance programs.

Each selected run case is built with an explicit compiler and stdlib (and the
conformance harness when the case asks for it), then run under `caged.py` on
sparrow-origin, sparrow-next and sparrow-core, sequentially. A case passes when
sparrow-core's stdout and exit status equal sparrow-origin's and, for failures,
its error text (without the program-name prefix) equals sparrow-next's. Test-mode
cases (`--test`) compare the exit status and the report with sparrow-next (the
origin CLI has no test mode). Program arguments, stdin and environment variables of a case are
passed through; cases needing networks, workers or process spawning are skipped
(the cage forbids them).
Cases outside the current core (unsupported construct) are reported apart.
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


def run(binary, spb, cwd, env, spec=None):
    spec = spec or {}
    flags = ['--test'] if spec.get('mode') == 'test' else []
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
    for prefix in ('sparrow-next: ', 'sparrow-core: ', 'sparrow-engine: '):
        if first.startswith(prefix):
            first = first[len(prefix):]
    return first


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--compiler', default='okkama')
    p.add_argument('--stdlib', type=Path, required=True)
    p.add_argument('--origin', type=Path, default=ROOT / 'sparrow-origin')
    p.add_argument('--next', type=Path, required=True)
    p.add_argument('--core', type=Path, required=True)
    p.add_argument('--work', type=Path, required=True)
    p.add_argument('levels', nargs='+', help='level directory prefixes, e.g. L01 L04')
    a = p.parse_args()
    env = dict(os.environ, OKKAMA_STDLIB_DIR=str(a.stdlib))
    a.work.mkdir(parents=True, exist_ok=True)
    totals = {'pass': 0, 'fail': 0, 'unsupported': 0, 'build': 0, 'skip': 0}
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
            built = subprocess.run([a.compiler, 'build', str(work / 'case' / spec['entry']),
                                    '--target', 'sparrow', '-o', str(spb)], env=env, capture_output=True)
            if built.returncode:
                totals['build'] += 1
                continue
            origin = run(a.origin, spb, work, env, spec)
            core = run(a.core, spb, work, env, spec)
            if 'sparrow-core yet' in core[2] or 'not supported by sparrow-core' in core[2] or 'asynchronous' in core[2]:
                totals['unsupported'] += 1
                print(f"UNSUPPORTED {spec['id']}: {message(core[2])[:110]}", flush=True)
                continue
            if spec.get('mode') == 'test':
                reference = run(a.next, spb, work, env, spec)
                # The origin CLI has no --test; the previous engine is the reference.
                ok = core[0] == reference[0] and core[1] == reference[1]
                detail = '' if ok else f' exit {reference[0]}/{core[0]} report same={core[1] == reference[1]}'
                totals['pass' if ok else 'fail'] += 1
                if not ok:
                    print(f"FAIL {spec['id']}:{detail}", flush=True)
                shutil.rmtree(work, ignore_errors=True)
                continue
            ok = core[0] == origin[0] and core[1] == origin[1]
            detail = ''
            if ok and core[0] != 0:
                reference = run(a.next, spb, work, env, spec)
                if message(reference[2]) != message(core[2]):
                    ok = False
                    detail = f' text: next={message(reference[2])[:80]!r} core={message(core[2])[:80]!r}'
            elif not ok:
                detail = (f' exit {origin[0]}/{core[0]} stdout same={core[1] == origin[1]}'
                          f' core: {message(core[2])[:90]!r}')
            totals['pass' if ok else 'fail'] += 1
            if not ok:
                print(f"FAIL {spec['id']}:{detail}", flush=True)
            shutil.rmtree(work, ignore_errors=True)
    print(' '.join(f'{k}={v}' for k, v in totals.items()))


if __name__ == '__main__':
    main()
