#!/usr/bin/env python3
"""Short sequential timing of prebuilt SPB programs: sparrow-origin versus sparrow-next.

Every run is bounded by probe_process (timeout, RSS, process tree). Output of the
new engine must match the reference byte for byte; a mismatch is reported and
the timing is not credited. Reports the median of --repeat runs per engine and,
with --stats, the `sparrow-next --stats` counters from one extra run.
This is a tool of the runtime overhead track, not the phase 10 comparison.
"""
import argparse
import hashlib
import statistics
import subprocess
import time
from pathlib import Path
from probe_process import run_bounded, ResourceLimitExceeded, positive_int


def timed(command, timeout):
    started = time.perf_counter()
    try:
        result = run_bounded(command, timeout=timeout)
    except (ResourceLimitExceeded, subprocess.TimeoutExpired) as error:
        return None, None, type(error).__name__ + (f" {error.args[0]}" if isinstance(error, ResourceLimitExceeded) else "")
    elapsed = time.perf_counter() - started
    if result.returncode != 0:
        detail = result.stderr.decode(errors='replace').strip().splitlines()
        return None, result.stdout, f"exit {result.returncode}: {detail[0][:100] if detail else ''}"
    return elapsed, result.stdout, None


def measure(command, repeat, timeout):
    times, output = [], None
    for _ in range(repeat):
        elapsed, out, error = timed(command, timeout)
        if error:
            return None, out, error
        if output is not None and out != output:
            return None, out, 'nondeterministic output'
        times.append(elapsed)
        output = out
    return statistics.median(times), output, None


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--origin', type=Path, required=True)
    p.add_argument('--next', type=Path, required=True)
    p.add_argument('--repeat', type=positive_int, default=3)
    p.add_argument('--timeout', type=positive_int, default=60)
    p.add_argument('--cold', action='store_true', help='also time sparrow-next --cold')
    p.add_argument('--stats', action='store_true', help='print sparrow-next --stats counters')
    p.add_argument('programs', type=Path, nargs='+')
    a = p.parse_args()
    engines = [('origin', [str(a.origin)]), ('next', [str(a.next)])]
    if a.cold:
        engines.append(('cold', [str(a.next), '--cold']))
    print(f"{'program':<20}" + ''.join(f'{name:>10}' for name, _ in engines) + '   ratio  output')
    for program in a.programs:
        row, reference, notes = [], None, []
        for name, command in engines:
            median, output, error = measure(command + [str(program)], a.repeat, a.timeout)
            if reference is None:
                reference = output
            elif error is None and output != reference:
                error = 'output differs from origin'
            row.append(median)
            if error:
                notes.append(f'{name}: {error}')
        cells = ''.join(f'{t:10.3f}' if t is not None else f"{'—':>10}" for t in row)
        ratio = f'{row[1] / row[0]:7.2f}x' if None not in row[:2] else f"{'—':>8}"
        digest = hashlib.sha256(reference or b'').hexdigest()[:8]
        print(f'{program.stem:<20}{cells} {ratio}  {digest} {"; ".join(notes)}', flush=True)
        if a.stats:
            try:
                result = run_bounded([str(a.next), '--stats', str(program)], timeout=a.timeout)
                for line in result.stderr.decode(errors='replace').splitlines():
                    print(f'    {line}')
            except (ResourceLimitExceeded, subprocess.TimeoutExpired) as error:
                print(f'    stats: {error}')


if __name__ == '__main__':
    main()
