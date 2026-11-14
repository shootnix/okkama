#!/usr/bin/env python3
"""Bounded ownership oracle for an installed Okkama toolchain (stdlib only)."""
import argparse
from collections import Counter, defaultdict
from concurrent.futures import ThreadPoolExecutor
import difflib
import hashlib
import json
import math
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import tempfile
import time

TEST = re.compile(r'^\s*(?:async\s+)?test\s+"', re.M)
CHECK = re.compile(r'^(not ok|ok)\b(?:\s+(\d+))?(.*)$')
PLAN = re.compile(r'^1\.\.(\d+)(?:\s*#.*)?$')
SPECIAL = {
    'panic-unwind.ok': r'(?m)^Panic: ownership corpus deliberate panic$',
    'secondary-failures.ok': (r'(?m)^Body: Body\.failed\nSecondary cleanup failure:\n'
                              r'[ \t]+Cleanup: Cleanup\.second\nSecondary cleanup failure:\n'
                              r'[ \t]+Cleanup: Cleanup\.first$'),
}
INSTRUMENTATION = ('SPARROW_TRACE_DESTROY',
                   'OKKAMA_OWNERSHIP_REPORT', 'SPARROW_CORE_STATS')


def positive(value):
    number = float(value)
    if not math.isfinite(number) or number <= 0:
        raise argparse.ArgumentTypeError('must be finite and positive')
    return number


def assignment(value):
    key, sep, val = value.partition('=')
    if not sep or not re.fullmatch(r'[A-Za-z_][A-Za-z_0-9]*', key):
        raise argparse.ArgumentTypeError('expected NAME=VALUE')
    if key in INSTRUMENTATION:
        raise argparse.ArgumentTypeError('instrumentation variables are managed by the oracle')
    return key, val


def parser():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('mode', choices=('engines', 'ab', 'stats', 'tap'))
    p.add_argument('--toolchain', type=Path, required=True, help='installed bin/ and lib/okkama/std')
    p.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[2])
    p.add_argument('--corpus', action='append', choices=('ownership', 'conformance', 'std', 't', 'bench'),
                   help='repeat to select corpora; default: all available')
    p.add_argument('--pending', action='store_true', help='include ownership pending/ separately')
    p.add_argument('--bench', type=Path, help='benchmark tree of **/main.ok, or a single .ok file')
    p.add_argument('--program-arg', action='append', default=[], help='argument for benchmark programs only')
    p.add_argument('--a-env', action='append', type=assignment, default=[], metavar='NAME=VALUE')
    p.add_argument('--b-env', action='append', type=assignment, default=[], metavar='NAME=VALUE')
    p.add_argument('--jobs', type=int, default=4)
    p.add_argument('--timeout', type=positive, default=60, help='seconds per execution (default: 60)')
    p.add_argument('--build-timeout', type=positive, default=120)
    p.add_argument('--out', type=Path, default=Path('oracle-results'), help='parent for a fresh run directory')
    p.add_argument('--json', type=Path, help='also write the report to this file')
    p.add_argument('--allow-host', action='store_true', help='explicitly bypass container guard')
    return p


def in_container():
    if Path('/.dockerenv').exists() or Path('/run/.containerenv').exists():
        return True
    try:
        return bool(re.search(r'docker|kubepods|containerd|libpod', Path('/proc/1/cgroup').read_text()))
    except OSError:
        return False


def discover(args):
    selected = args.corpus or ['ownership', 'conformance', 'std', 't'] + (['bench'] if args.bench else [])
    programs = []

    def add(corpus, paths, tests=False, rejected=False):
        for path in sorted(paths):
            programs.append(dict(corpus=corpus, path=str(path.resolve()), tests=tests, rejected=rejected))

    for corpus in dict.fromkeys(selected):
        if corpus == 'bench':
            if args.bench is None or not args.bench.exists():
                raise ValueError('--corpus bench requires an existing --bench path')
            add(corpus, [args.bench] if args.bench.is_file() else args.bench.rglob('main.ok'))
            continue
        base = args.root / {'ownership': 'tests/ownership-corpus', 'conformance': 'conformance/cases',
                            'std': 'std', 't': 't'}[corpus]
        if not base.is_dir():
            raise ValueError(f'missing corpus directory: {base}')
        if corpus == 'ownership':
            add(corpus, base.glob('*.ok'))
            add('rejected', (base / 'rejected').rglob('*.ok'), rejected=True)
            if args.pending:
                add('pending', (base / 'pending').rglob('*.ok'))
        elif corpus == 'std':
            add(corpus, (p for p in base.rglob('*.ok') if TEST.search(p.read_text())), tests=True)
        elif corpus == 't':
            add(corpus, (p for p in base.rglob('*.ok') if 'support' != p.relative_to(base).parts[0]))
        else:
            add(corpus, base.rglob('main.ok'))
    for item in programs:
        path = Path(item['path'])
        base = args.bench if item['corpus'] == 'bench' else args.root
        item['name'] = str(path.relative_to(base)) if base.is_dir() else path.name
        item['id'] = item['corpus'] + '/' + item['name']
    return programs


def read(path):
    return Path(path).read_text(encoding='utf-8', errors='replace')


def execute(command, env, cwd, prefix, timeout):
    """Use files, closed stdin and a new process group; kill descendants on exit/timeout."""
    result = dict(command=[str(x) for x in command], cwd=str(cwd), stdout=str(prefix) + '.out',
                  stderr=str(prefix) + '.err', timeout=False, error=None, code=None)
    start = time.monotonic()
    with open(result['stdout'], 'wb') as out, open(result['stderr'], 'wb') as err:
        try:
            child = subprocess.Popen(result['command'], cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                                     stdout=out, stderr=err, start_new_session=True)
            try:
                result['code'] = child.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                result['timeout'] = True
            finally:
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                child.wait()
        except OSError as exc:
            result['error'] = str(exc)
    result['seconds'] = round(time.monotonic() - start, 3)
    return result


def healthy(result):
    return not result['timeout'] and result['error'] is None and result['code'] is not None and result['code'] >= 0


def fingerprint(path):
    digest = hashlib.sha256()
    with open(path, 'rb') as stream:
        for chunk in iter(lambda: stream.read(65536), b''):
            digest.update(chunk)
    return digest.hexdigest()


def trace_blocks(path):
    """Keep exact bytes, test order, duplicate names and the pre-test prefix."""
    blocks = [bytearray()]
    with open(path, 'rb') as stream:
        for line in stream:
            if line.startswith(b'# test '):
                blocks.append(bytearray())
            blocks[-1].extend(line)
    return blocks


def trace_diff(left, right):
    a, b = trace_blocks(left), trace_blocks(right)
    changes = []
    for index in range(max(len(a), len(b))):
        x = a[index] if index < len(a) else None
        y = b[index] if index < len(b) else None
        if x == y:
            continue
        changes.append(f'trace block {index} differs (0 = pre-test prefix)')
        changes.extend(difflib.unified_diff(
            bytes(x or b'').decode('utf-8', errors='replace').splitlines(),
            bytes(y or b'').decode('utf-8', errors='replace').splitlines(),
            fromfile=f'A/trace/block-{index}', tofile=f'B/trace/block-{index}', n=2))
        if len(changes) >= 40:
            break
    return changes[:40]


def compare(left, right):
    changes = {}
    for field in ('code', 'stdout', 'trace'):
        a, b = left[field], right[field]
        equal = a == b if field == 'code' else fingerprint(a) == fingerprint(b)
        if not equal:
            if field == 'code':
                changes[field] = [a, b]
            elif field == 'trace':
                changes[field] = trace_diff(a, b)
            else:
                # Exact comparison above; short human-readable excerpt below.
                changes[field] = list(difflib.unified_diff(read(a).splitlines(), read(b).splitlines(),
                                                         fromfile='A/' + field, tofile='B/' + field,
                                                         n=2))[:40]
    return changes


def tap_report(run, required=False):
    lines = read(run['stdout']).splitlines()
    checks = [(i, CHECK.match(line)) for i, line in enumerate(lines) if CHECK.match(line)]
    plans = [int(m.group(1)) for line in lines if (m := PLAN.match(line))]
    bailouts = [line for line in lines if line.lower().startswith('bail out!')]
    active = bool(checks or plans or required or bailouts)
    report = dict(active=active, ok=0, not_ok=0, skipped=0, todo=0, failures=[], problems=[])
    if not active:
        return report
    for ordinal, (index, match) in enumerate(checks, 1):
        failed = match.group(1) == 'not ok'
        report['not_ok' if failed else 'ok'] += 1
        directive = re.search(r'#\s*(SKIP|TODO)\b', match.group(3), re.I)
        if directive:
            report['skipped' if directive.group(1).upper() == 'SKIP' else 'todo'] += 1
        if match.group(2) and int(match.group(2)) != ordinal:
            report['problems'].append(f'out of order: {lines[index]}')
        if failed and not directive:
            # Ownership diagnostics precede the assertion; TAP YAML usually follows it.
            lo = index
            while lo > 0 and lines[lo - 1].lstrip().startswith('#'):
                lo -= 1
            hi = checks[ordinal][0] if ordinal < len(checks) else len(lines)
            report['failures'].append('\n'.join(lines[lo:hi])[:6000])
    if len(plans) != 1 or plans[0] != len(checks):
        report['problems'].append(f'plan {plans}, observed {len(checks)} checks')
    report['problems'].extend(bailouts)
    return report


def flatten_numbers(value, prefix=''):
    result = Counter()
    for key, val in value.items():
        name = prefix + key
        if isinstance(val, dict):
            result.update(flatten_numbers(val, name + '.'))
        elif isinstance(val, int) and not isinstance(val, bool):
            result[name] += val
    return result


def core_stats(text):
    counts = Counter()
    for line in text.splitlines():
        match = re.fullmatch(r'\s*(\d+)\s+(helper region_\S+|helper created|region\.enter, count)\s*', line)
        if match:
            counts[match.group(2)] += int(match.group(1))
    return dict(counts)


def process(item, args, output, environments):
    folder = output / hashlib.sha256(item['id'].encode()).hexdigest()[:20]
    folder.mkdir()
    result = dict(item, artifacts=str(folder), builds={}, runs={}, differences={}, problems=[], stats={})
    ok = args.toolchain / 'bin/okkama'
    sp = args.toolchain / 'bin/sparrow'
    source = Path(item['path'])
    flags = ['-t'] if item['tests'] else []
    variants = ('A', 'B') if args.mode in ('ab', 'stats') else ('A',)
    for variant in variants:
        census = folder / (variant + '.census.json')
        env = dict(environments[variant], OKKAMA_OWNERSHIP_REPORT=str(census))
        command = [ok, 'build', *flags, source, '--target', 'sparrow', '-o', folder / (variant + '.spb')]
        result['builds'][variant] = execute(command, env, source.parent, folder / (variant + '.build'), args.build_timeout)
    builds = list(result['builds'].values())
    if item['rejected']:
        # Compiler documents exit 1 for source/lowering/codegen errors; crashes,
        # timeouts, usage (2) and I/O (3) must not count as expected rejection.
        result['status'] = 'rejected' if all(healthy(b) and b['code'] == 1 for b in builds) else 'rejection-fail'
        return result
    if any(not healthy(b) or b['code'] != 0 for b in builds):
        result['status'] = 'build-fail'
        return result
    labels = [('source', 'A'), ('sparrow', 'A')] if args.mode in ('engines', 'tap') else [('A', 'A'), ('A2', 'A'), ('B', 'B')]
    for label, variant in labels:
        trace = folder / (label + '.trace')
        trace.touch()
        env = dict(environments[variant], SPARROW_TRACE_DESTROY=str(trace))
        if args.mode == 'stats':
            env['SPARROW_CORE_STATS'] = '1'
        program_args = args.program_arg if item['corpus'] == 'bench' else []
        command = ([ok, 'run', '--engine', 'sparrow', *flags, source, *program_args] if label == 'source' else
                   [sp, *(['test'] if item['tests'] else []), folder / (variant + '.spb'), *program_args])
        run = execute(command, env, source.parent, folder / label, args.timeout)
        run['trace'] = str(trace)
        run['tap'] = tap_report(run, item['corpus'] in ('ownership', 'pending', 't'))
        result['runs'][label] = run
        if not healthy(run):
            result['problems'].append(f'{label}: timeout, signal or launch error')
        pattern = SPECIAL.get(source.name) if item['corpus'] == 'ownership' else None
        if pattern and (run['code'] != 1 or not re.search(pattern, read(run['stderr']))):
            result['problems'].append(f'{label}: expected runtime-failure contract violated')
        if args.mode == 'tap' and not pattern and run['code'] != 0:
            result['problems'].append(f'{label}: exit {run["code"]}')
    runs = result['runs']
    if args.mode in ('ab', 'stats'):
        result['determinism_diff'] = compare(runs['A'], runs['A2'])
        result['differences'] = compare(runs['A'], runs['B'])
        result['status'] = 'nondeterministic' if result['determinism_diff'] else ('DIFF' if result['differences'] else 'same')
    else:
        result['differences'] = compare(runs['source'], runs['sparrow'])
        result['status'] = 'DIFF' if result['differences'] else 'same'
    if args.mode == 'stats':
        for variant in variants:
            stderr = read(runs[variant]['stderr'])
            entry = dict(helpers=core_stats(stderr), census={})
            try:
                data = json.loads((folder / (variant + '.census.json')).read_text())
                totals = data['totals']
                for key in ('scope', 'result', 'aggregate', 'unknown'):
                    if not isinstance(totals[key], int):
                        raise ValueError(f'invalid census {key}')
                entry['census'] = dict(flatten_numbers(totals))
            except (OSError, ValueError, KeyError, TypeError) as exc:
                result['problems'].append(f'{variant}: missing/invalid census: {exc}')
            # Zero-call helpers are omitted by Sparrow. The unconditional heap
            # counter distinguishes a genuine zero from a missing stats report.
            entry['report_present'] = bool(re.search(r'^\s*\d+\s+heap bytes allocated\s*$', stderr, re.M))
            if not entry['report_present']:
                result['problems'].append(f'{variant}: no core statistics report found')
            result['stats'][variant] = entry
    if args.mode == 'tap':
        if any(r['tap']['failures'] or r['tap']['problems'] for r in runs.values()):
            result['problems'].append('TAP failures')
    if result['problems'] and result['status'] == 'same':
        result['status'] = 'run-fail'
    return result


def summarize(results):
    counts = defaultdict(Counter)
    taps = defaultdict(Counter)
    stats = defaultdict(lambda: {'helpers': Counter(), 'census': Counter(), 'programs': 0})
    for result in results:
        corpus = result['corpus']
        for status in ('same', 'DIFF', 'nondeterministic', 'build-fail'):
            counts[corpus].setdefault(status, 0)
        counts[corpus][result['status']] += 1
        for label, run in result['runs'].items():
            if run['tap']['active']:
                key = corpus + '/' + label
                taps[key]['programs'] += 1
                for field in ('ok', 'not_ok', 'skipped', 'todo'):
                    taps[key][field] += run['tap'][field]
        # Only matched, healthy A/B runs contribute; A2 is never double counted.
        if result['stats'] and not result['problems'] and result['status'] != 'nondeterministic':
            for label, entry in result['stats'].items():
                for key in (corpus + '/' + label, 'TOTAL/' + label):
                    stats[key]['programs'] += 1
                    for field in ('helpers', 'census'):
                        stats[key][field].update(entry[field])
    return dict(counts=counts, tap=taps, stats=stats)


def main():
    p = parser()
    args = p.parse_args()
    if args.jobs < 1:
        p.error('--jobs must be positive')
    if not args.allow_host and not in_container():
        p.error('refusing to execute outside a container; use oracle-docker.sh or explicitly --allow-host')
    args.root = args.root.resolve()
    args.toolchain = args.toolchain.resolve()
    if args.bench:
        args.bench = args.bench.resolve()
    for name in ('okkama', 'sparrow'):
        if not os.access(args.toolchain / 'bin' / name, os.X_OK):
            p.error(f'missing executable: {args.toolchain}/bin/{name}')
    if not (args.toolchain / 'lib/okkama/std').is_dir():
        p.error('missing installed lib/okkama/std')
    if args.b_env and args.mode in ('engines', 'tap'):
        p.error('--b-env applies only to ab and stats')
    try:
        programs = discover(args)
    except (OSError, ValueError) as exc:
        p.error(str(exc))
    if not programs:
        p.error('no programs discovered')
    args.out.mkdir(parents=True, exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix=args.mode + '-', dir=args.out.resolve()))
    baseline = {key: value for key, value in os.environ.items() if key not in INSTRUMENTATION}
    baseline.update(OKKAMA_STDLIB_DIR=str(args.toolchain / 'lib/okkama/std'),
                    OKKAMA_IMPORT_DIRS=str(args.root / 't/support'))
    environments = {name: dict(baseline, **dict(overrides)) for name, overrides in (('A', args.a_env), ('B', args.b_env))}
    print(f'{args.mode}: {len(programs)} programs, {args.jobs} jobs; artifacts: {output}', flush=True)
    results = []
    with ThreadPoolExecutor(max_workers=args.jobs) as pool:
        for result in pool.map(lambda item: process(item, args, output, environments), programs):
            results.append(result)
            print(f'{result["status"]:16} {result["id"]}', flush=True)
    summary = summarize(results)
    report = dict(schema=1, mode=args.mode, root=str(args.root), toolchain=str(args.toolchain),
                  overrides={'A': dict(args.a_env), 'B': dict(args.b_env)},
                  settings=dict(jobs=args.jobs, timeout=args.timeout, build_timeout=args.build_timeout,
                                pending=args.pending, program_args=args.program_arg),
                  artifacts=str(output), summary=summary,
                  differing=[r['id'] for r in results if r['differences']],
                  nondeterministic=[r['id'] for r in results if r.get('determinism_diff')], programs=results)
    payload = json.dumps(report, indent=2, ensure_ascii=True) + '\n'
    (output / 'report.json').write_text(payload)
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(payload)
    for corpus, counts in sorted(summary['counts'].items()):
        print(corpus + ': ' + ', '.join(f'{key}={value}' for key, value in sorted(counts.items())))
    if args.mode in ('tap', 'stats'):
        print(json.dumps(summary['tap' if args.mode == 'tap' else 'stats'], indent=2))
    for result in results:
        if result['status'] in ('same', 'rejected') and not result['problems']:
            continue
        print('\n' + result['id'] + ': ' + result['status'])
        for problem in result['problems']:
            print('  ' + problem)
        for key in ('determinism_diff', 'differences'):
            for field, diff in result.get(key, {}).items():
                print(f'  {key}/{field}:\n' + '\n'.join(str(line) for line in diff))
        for label, run in {**result['builds'], **result['runs']}.items():
            if label in result['runs'] and args.mode == 'tap':
                for detail in run['tap']['failures'] + run['tap']['problems']:
                    print(f'  {label}: {detail}')
            if not healthy(run) or run['code'] != 0:
                print(f'  {label}: code={run["code"]} timeout={run["timeout"]} error={run["error"]}')
                print('\n'.join(read(run['stdout']).splitlines()[-20:]))
                print('\n'.join(read(run['stderr']).splitlines()[-20:]))
    print(f'Report: {output / "report.json"}')
    # Positive build failures are coverage gaps, never a fatal suite result.
    return int(any(r['status'] not in ('same', 'rejected', 'build-fail') or r['problems'] for r in results))


if __name__ == '__main__':
    sys.exit(main())
