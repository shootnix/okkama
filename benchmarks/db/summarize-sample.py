"""Summarize main-thread sample stacks without counting recursion twice.

Usage: python3 benchmarks/db/summarize-sample.py
Categories overlap; percentages must not be added. No allocation counts inferred.
"""
import gzip
import re
from pathlib import Path

path = Path(__file__).parent / 'results/prepared-inline.sample.txt.gz'
source = gzip.decompress(path.read_bytes()).decode()
graph = source.split('Call graph:\n')[1].split('Total number in stack')[0]
nodes, stack = [], []
for line in graph.splitlines():
    match = re.match(r'([ +|!:]*)(\d+) (.*)', line)
    if not match:
        continue
    depth, count, symbol = len(match[1]), int(match[2]), match[3]
    while stack and stack[-1]['depth'] >= depth:
        stack.pop()
    if stack:
        stack[-1]['self'] -= count
    node = dict(depth=depth, count=count, self=count, symbol=symbol,
                ancestors=[n['symbol'] for n in stack])
    nodes.append(node)
    stack.append(node)
assert all(n['self'] >= 0 for n in nodes)
main = [n for n in nodes if any('com.apple.main-thread' in s
        for s in n['ancestors'] + [n['symbol']])]
total = sum(n['self'] for n in main)
patterns = {
    'Region methods': r'6Region',
    'heap::collect': r'4heap7collect',
    'Allocator symbols': r'\b_?mi_|\bmalloc|\bfree\b',
    'Value::clone': r'5Value.*5clone5Clone5clone',
    'memcpy/memmove': r'memcpy|memmove',
    'sqlite3 symbols': r'sqlite3',
    'call_generic': r'12call_generic',
}
print('Main-thread observations:', total)
for label, pattern in patterns.items():
    own = sum(n['self'] for n in main if re.search(pattern, n['symbol']))
    inclusive = sum(n['self'] for n in main if any(re.search(pattern, s)
                    for s in n['ancestors'] + [n['symbol']]))
    print(f'{label}: self={own}, inclusive={inclusive} ({100*inclusive/total:.2f}%)')
