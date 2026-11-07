"""Bounded POSIX subprocesses for the new-runtime probes.

No preexec_fn: probes may use threads. RSS/process counts are sampled, not hard
OS quotas. ps must be available; a missing monitor fails closed before launch.
"""
import os
from collections import deque
from concurrent.futures import ThreadPoolExecutor
from itertools import islice
import selectors
import signal
import subprocess
import threading
import time

_OUTPUT_LOCK = threading.Lock()
_OUTPUT_CAPTURED = 0
_STOP = threading.Event()


def map_bounded(function, items, jobs):
    """Queue at most jobs cases; interruption cancels running process probes."""
    items = iter(items)
    with ThreadPoolExecutor(max_workers=jobs) as pool:
        pending = deque(pool.submit(function, item) for item in islice(items, jobs))
        try:
            while pending:
                yield pending.popleft().result()
                for item in islice(items, 1):
                    pending.append(pool.submit(function, item))
        except BaseException:
            _STOP.set()
            for future in pending:
                future.cancel()
            raise

class ResourceLimitExceeded(subprocess.SubprocessError):
    def __init__(self, reason, stdout=b'', stderr=b''):
        super().__init__(reason)
        self.reason = reason
        self.stdout = stdout
        self.stderr = stderr


def positive_int(value):
    number = int(value)
    if number <= 0:
        raise ValueError('limit must be positive')
    return number


def process_snapshot():
    result = subprocess.run(
        ['/bin/ps', '-axo', 'pid=,ppid=,pgid=,rss=,lstart='],
        stdin=subprocess.DEVNULL, capture_output=True, timeout=2, check=True,
    )
    processes = {}
    for line in result.stdout.decode().splitlines():
        pid, parent, group, rss, started = line.split(maxsplit=4)
        processes[int(pid)] = (int(parent), int(group), int(rss), started)
    return processes


def track_tree(snapshot, root, known):
    # Keep identities across reparenting, without signalling a reused PID.
    active = {pid for pid, identity in known.items()
              if pid in snapshot and snapshot[pid][3] == identity}
    if root in snapshot and (root not in known or root in active):
        active.add(root)
    root_reused = (root in known and root in snapshot
                   and snapshot[root][3] != known[root])
    while True:
        added = {pid for pid, (parent, group, _, _) in snapshot.items()
                 if parent in active or (group == root and not root_reused)} - active
        if not added:
            break
        active.update(added)
    for pid in active:
        known[pid] = snapshot[pid][3]
    return active


def terminate_tree(process, known):
    monitored = True
    try:
        snapshot = process_snapshot()
        active = track_tree(snapshot, process.pid, known)
    except (OSError, subprocess.SubprocessError):
        # The owned group can still be stopped if monitoring fails mid-run.
        snapshot, active = {}, set()
        monitored = False
    # Workers may create their own process groups inside the probe's session.
    groups = {snapshot[pid][1] for pid in active if snapshot[pid][1] in active}
    root_reused = (process.pid in known and process.pid in snapshot
                   and snapshot[process.pid][3] != known[process.pid])
    if not root_reused and (monitored or process.poll() is None):
        groups.add(process.pid)
    for group in groups:
        try:
            os.killpg(group, signal.SIGKILL)
        except ProcessLookupError:
            pass
    for pid in active:
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    if process.poll() is None:
        process.kill()
    process.wait(timeout=5)


def run_bounded(command, *, timeout, cwd=None, env=None, input=None,
                stdin=None, capture_output=True):
    global _OUTPUT_CAPTURED
    if _STOP.is_set():
        raise InterruptedError('probe interrupted')
    if not capture_output or stdin not in (None, subprocess.DEVNULL):
        raise ValueError('probe runner requires captured output and byte input')
    if timeout <= 0:
        raise ValueError('timeout must be positive')
    output_limit = positive_int(os.environ.get('SPARROW_PROBE_OUTPUT_BYTES', 262144))
    total_output_limit = positive_int(os.environ.get('SPARROW_PROBE_TOTAL_OUTPUT_BYTES', 16 * 1024 * 1024))
    memory_kib = positive_int(os.environ.get('SPARROW_PROBE_MEMORY_MB', 1024)) * 1024
    process_limit = positive_int(os.environ.get('SPARROW_PROBE_MAX_PROCESSES', 16))
    with _OUTPUT_LOCK:
        if _OUTPUT_CAPTURED >= total_output_limit:
            raise ResourceLimitExceeded('REPORT_OUTPUT_LIMIT')
    process_snapshot()  # Fail before launching if RSS/tree monitoring is unavailable.
    known = {}
    stdout, stderr = bytearray(), bytearray()
    deadline = time.monotonic() + timeout
    process = subprocess.Popen(
        command, cwd=cwd, env=env, start_new_session=True,
        stdin=subprocess.PIPE if input else subprocess.DEVNULL,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    )
    try:
        with selectors.DefaultSelector() as selector:
            for stream, target in [(process.stdout, stdout), (process.stderr, stderr)]:
                os.set_blocking(stream.fileno(), False)
                selector.register(stream, selectors.EVENT_READ, target)
            pending = memoryview(input or b'')
            if process.stdin is not None:
                os.set_blocking(process.stdin.fileno(), False)
                selector.register(process.stdin, selectors.EVENT_WRITE, None)
            next_monitor = 0
            while selector.get_map() or process.poll() is None:
                if _STOP.is_set():
                    raise InterruptedError('probe interrupted')
                now = time.monotonic()
                if now >= deadline:
                    raise subprocess.TimeoutExpired(command, timeout, bytes(stdout), bytes(stderr))
                if now >= next_monitor:
                    snapshot = process_snapshot()
                    active = track_tree(snapshot, process.pid, known)
                    if len(active) > process_limit:
                        raise ResourceLimitExceeded('PROCESS_LIMIT', bytes(stdout), bytes(stderr))
                    if sum(snapshot[pid][2] for pid in active) > memory_kib:
                        raise ResourceLimitExceeded('MEMORY_LIMIT', bytes(stdout), bytes(stderr))
                    next_monitor = now + 0.1
                for key, _ in selector.select(min(0.05, max(0, deadline - now))):
                    stream, target = key.fileobj, key.data
                    if target is None:
                        try:
                            pending = pending[os.write(stream.fileno(), pending[:8192]):]
                        except BrokenPipeError:
                            pending = pending[:0]
                        except BlockingIOError:
                            continue
                        if not pending:
                            selector.unregister(stream)
                            stream.close()
                    else:
                        try:
                            chunk = os.read(stream.fileno(), 8192)
                        except BlockingIOError:
                            continue
                        if not chunk:
                            selector.unregister(stream)
                            stream.close()
                        else:
                            left = output_limit - len(stdout) - len(stderr)
                            with _OUTPUT_LOCK:
                                report_left = max(0, total_output_limit - _OUTPUT_CAPTURED)
                                captured = chunk[:min(left, report_left)]
                                _OUTPUT_CAPTURED += len(captured)
                            target.extend(captured)
                            if len(chunk) > len(captured):
                                reason = 'REPORT_OUTPUT_LIMIT' if report_left < left else 'OUTPUT_LIMIT'
                                raise ResourceLimitExceeded(reason, bytes(stdout), bytes(stderr))
            return subprocess.CompletedProcess(command, process.returncode, bytes(stdout), bytes(stderr))
    finally:
        # Also stop descendants left behind by a successful/failed parent.
        try:
            terminate_tree(process, known)
        finally:
            for stream in (process.stdin, process.stdout, process.stderr):
                if stream is not None:
                    stream.close()
