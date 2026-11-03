import os
import subprocess
import sys
import unittest
from unittest.mock import Mock, patch

import probe_process as runner


class ProbeProcessTests(unittest.TestCase):
    def setUp(self):
        self.counter = patch.object(runner, '_OUTPUT_CAPTURED', 0)
        self.counter.start()
        self.addCleanup(self.counter.stop)

    def run_script(self, source, **kwargs):
        # Exercise real pipes/process groups without requiring macOS ps sandbox
        # privileges. Tree discovery and RSS enforcement have separate tests.
        with patch.object(runner, 'process_snapshot', return_value={}):
            return runner.run_bounded([sys.executable, '-c', source], timeout=2, **kwargs)

    def test_binary_input_and_both_output_streams(self):
        result = self.run_script(
            'import sys; sys.stdout.buffer.write(sys.stdin.buffer.read()); '
            'sys.stderr.buffer.write(b"diagnostic")', input=b'hello\x00world',
        )
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout, b'hello\x00world')
        self.assertEqual(result.stderr, b'diagnostic')

    def test_output_overflow_is_bounded_and_not_a_timeout(self):
        with patch.dict(os.environ, {'SPARROW_PROBE_OUTPUT_BYTES': '4096'}):
            with self.assertRaises(runner.ResourceLimitExceeded) as caught:
                self.run_script('import os; os.write(1, b"x" * 8192)')
        self.assertEqual(caught.exception.reason, 'OUTPUT_LIMIT')
        self.assertLessEqual(len(caught.exception.stdout) + len(caught.exception.stderr), 4096)

    def test_timeout_stops_inherited_pipes(self):
        with patch.object(runner, 'process_snapshot', return_value={}):
            with self.assertRaises(subprocess.TimeoutExpired):
                runner.run_bounded(
                    [sys.executable, '-c',
                     'import subprocess,sys,time; '
                     'subprocess.Popen([sys.executable,"-c","import time; time.sleep(3)"]); '
                     'time.sleep(3)'], timeout=0.2,
                )

    def test_report_output_budget_is_shared_across_runs(self):
        with patch.dict(os.environ, {'SPARROW_PROBE_TOTAL_OUTPUT_BYTES': '5'}):
            self.assertEqual(self.run_script('print("abc", end="")').stdout, b'abc')
            with self.assertRaises(runner.ResourceLimitExceeded) as caught:
                self.run_script('print("def", end="")')
            self.assertEqual(caught.exception.reason, 'REPORT_OUTPUT_LIMIT')
            self.assertEqual(caught.exception.stdout, b'de')
            with patch.object(runner.subprocess, 'Popen') as launch:
                with self.assertRaises(runner.ResourceLimitExceeded):
                    runner.run_bounded(['must-not-launch'], timeout=1)
                launch.assert_not_called()

    def test_missing_monitor_does_not_launch_program(self):
        with patch.object(runner, 'process_snapshot', side_effect=OSError('no monitor')):
            with patch.object(runner.subprocess, 'Popen') as launch:
                with self.assertRaises(OSError):
                    runner.run_bounded(['must-not-launch'], timeout=1)
                launch.assert_not_called()

    def test_descendants_in_other_groups_and_reparented_children_are_tracked(self):
        known = {}
        snapshot = {10: (1, 10, 20, 'a'), 11: (10, 11, 30, 'b'),
                    12: (11, 11, 40, 'c'), 99: (1, 99, 50, 'd')}
        self.assertEqual(runner.track_tree(snapshot, 10, known), {10, 11, 12})
        del snapshot[10]
        snapshot[11] = (1, 11, 30, 'b')
        self.assertEqual(runner.track_tree(snapshot, 10, known), {11, 12})
        snapshot[12] = (1, 12, 40, 'reused pid')
        self.assertEqual(runner.track_tree(snapshot, 10, known), {11})

    def test_memory_limit_terminates_process(self):
        launched = []
        original = subprocess.Popen

        def launch(*args, **kwargs):
            process = original(*args, **kwargs)
            launched.append(process)
            return process

        def snapshot():
            return {p.pid: (os.getpid(), p.pid, 2048, 'test identity') for p in launched}

        with patch.dict(os.environ, {'SPARROW_PROBE_MEMORY_MB': '1'}):
            with patch.object(runner, 'process_snapshot', side_effect=snapshot):
                with patch.object(runner.subprocess, 'Popen', side_effect=launch):
                    with self.assertRaises(runner.ResourceLimitExceeded) as caught:
                        runner.run_bounded(
                            [sys.executable, '-c', 'import time; time.sleep(3)'], timeout=2,
                        )
        self.assertEqual(caught.exception.reason, 'MEMORY_LIMIT')
        self.assertIsNotNone(launched[0].returncode)

    def test_reused_root_pid_and_its_group_are_not_terminated(self):
        known = {10: 'original root'}
        snapshot = {10: (1, 10, 20, 'unrelated process'),
                    11: (10, 10, 30, 'unrelated child')}
        self.assertEqual(runner.track_tree(snapshot, 10, known), set())
        process = Mock(pid=10)
        process.poll.return_value = 0
        with patch.object(runner, 'process_snapshot', return_value=snapshot):
            with patch.object(runner.os, 'killpg') as group_signal:
                with patch.object(runner.os, 'kill') as process_signal:
                    runner.terminate_tree(process, known)
                    group_signal.assert_not_called()
                    process_signal.assert_not_called()
        process.kill.assert_not_called()


if __name__ == '__main__':
    unittest.main()
