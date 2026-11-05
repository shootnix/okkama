#!/usr/bin/env python3
"""Run one command in a local cage for risky async/stress probes on macOS.

Layers, each bounding a different failure:
- taskpolicy -b: background band — efficiency cores, low CPU/IO priority, so a
  runaway cannot take the performance cores or make the machine unresponsive;
- sandbox-exec: no process creation (fork/exec of children), no network, writes
  only under --writable directories and /dev;
- ulimit -t: hard CPU-seconds limit enforced by the kernel;
- probe_process.run_bounded: wall-clock timeout, RSS limit of the process tree,
  bounded captured output, kill of the whole process group.

This is not a VM: it does not cap the kernel resources a single process can use
beyond these limits. Docker with --cpus/--memory/--pids-limit is stronger when
its daemon is running; this cage is the default when it is not.
"""
import argparse
import os
import subprocess
import sys
from pathlib import Path
from probe_process import run_bounded, ResourceLimitExceeded, positive_int


def profile(writable):
    allowed = ''.join(f' (subpath "{os.path.realpath(p)}")' for p in writable)
    return f"""(version 1)
(allow default)
(deny network*)
(deny process-fork)
(deny file-write*)
(allow file-write* (subpath "/dev"){allowed})
"""


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--cpu-seconds', type=positive_int, default=30)
    p.add_argument('--timeout', type=positive_int, default=60)
    p.add_argument('--memory-mb', type=positive_int, default=1024)
    p.add_argument('--writable', type=Path, action='append', default=[])
    p.add_argument('command', nargs=argparse.REMAINDER)
    a = p.parse_args()
    command = a.command[1:] if a.command[:1] == ['--'] else a.command
    if not command:
        p.error('missing command')
    shell = f'ulimit -t {a.cpu_seconds}; exec "$@"'
    caged = ['taskpolicy', '-b', 'sandbox-exec', '-p', profile(a.writable),
             '/bin/sh', '-c', shell, 'caged'] + command
    os.environ['SPARROW_PROBE_MEMORY_MB'] = str(a.memory_mb)
    try:
        result = run_bounded(caged, timeout=a.timeout)
    except subprocess.TimeoutExpired:
        print('caged: wall-clock timeout', file=sys.stderr)
        return 124
    except ResourceLimitExceeded as error:
        print(f'caged: {error.args[0]}', file=sys.stderr)
        return 125
    sys.stdout.buffer.write(result.stdout)
    sys.stderr.buffer.write(result.stderr)
    if result.returncode < 0 or result.returncode in (152, 137):
        print(f'caged: killed (status {result.returncode}; CPU limit or signal)', file=sys.stderr)
    return result.returncode


if __name__ == '__main__':
    sys.exit(main())
