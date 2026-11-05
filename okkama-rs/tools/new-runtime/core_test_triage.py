#!/usr/bin/env python3
"""Classify sparrow-engine tests for the core rebuild (docs/new-runtime/core-rebuild.md).

Each #[test] function is assigned one category from the APIs and modules it uses:
- internal: observes the flat dispatcher (step-by-step resume, continuation
  depth, saved values, live registers, plan rewrites, fixed GC trigger);
  rewrite for the new model or drop when the property no longer exists;
- worker: launches process workers (Docker/VM only);
- native: unit tests of a native provider; move with the provider;
- heap: unit tests of values, heap and collector; port in stage 1;
- semantic: program behaviour (result, output, failure); port as is.
Prints a Markdown table per file and a summary.
"""
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2] / 'crates/sparrow-engine'
INTERNAL = re.compile(
    r'resume\(\s*[12]\s*\)|continuation_depth|saved_value_count|live_registers|live_integer'
    r'|elided_object_count|recurrence_loop_count|stress_collection|collection_count'
    r'|jit::with_failed_region|REGION_SIZE')
WORKER = re.compile(r'worker|Worker|process_host|spawn_worker|SPARROW_INTERNAL_WORKER')
HEAP_MODULES = {'value', 'arena', 'array', 'text', 'map', 'failure', 'equality', 'range',
                'rational', 'scalar', 'resource', 'liveness', 'validate', 'ordering', 'plan'}


def tests(path):
    text = path.read_text()
    starts = [m.start() for m in re.finditer(r'#\[test\]', text)]
    for i, start in enumerate(starts):
        body = text[start:starts[i + 1] if i + 1 < len(starts) else len(text)]
        name = re.search(r'fn\s+(\w+)', body)
        yield (name.group(1) if name else '?'), body


def classify(path, body):
    module = path.stem
    integration = path.parent.name == 'tests'
    if WORKER.search(body) and (integration or module.startswith('worker')):
        return 'worker'
    if INTERNAL.search(body):
        return 'internal'
    if not integration and any(part.startswith('native') for part in path.parts[-2:]):
        return 'native'
    if not integration and module in HEAP_MODULES:
        return 'heap'
    return 'semantic'


def main():
    files = sorted((ROOT / 'tests').glob('*.rs')) + sorted((ROOT / 'src').rglob('*.rs'))
    total = Counter()
    rows = defaultdict(Counter)
    names = defaultdict(lambda: defaultdict(list))
    for path in files:
        for name, body in tests(path):
            category = classify(path, body)
            key = str(path.relative_to(ROOT))
            rows[key][category] += 1
            names[key][category].append(name)
            total[category] += 1
    order = ['semantic', 'heap', 'native', 'internal', 'worker']
    print('| Файл | ' + ' | '.join(order) + ' |')
    print('|---|' + '---:|' * len(order))
    for key in sorted(rows):
        print(f'| `{key}` | ' + ' | '.join(str(rows[key][c] or '') for c in order) + ' |')
    print(f"| **всего {sum(total.values())}** | " + ' | '.join(f'**{total[c]}**' for c in order) + ' |')
    if '--internal' in sys.argv:
        print('\nВнутренние тесты:')
        for key in sorted(names):
            for name in names[key]['internal']:
                print(f'- `{key}`: `{name}`')


if __name__ == '__main__':
    main()
