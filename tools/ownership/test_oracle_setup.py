"""Unit tests for conformance case preparation; no compiler or Docker needed."""
import json
from pathlib import Path
import tempfile
import unittest

import oracle


class PrepareCaseTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.case = self.root / 'conformance' / 'cases' / 'L00-example' / 'sample'
        self.case.mkdir(parents=True)
        (self.case / 'main.ok').write_text('program source')
        (self.case / 'support.ok').write_text('support source')
        (self.case / 'input.txt').write_bytes(b'input bytes')
        data = {
            'schema': 'ocs-case/2', 'sources': ['main.ok'], 'entry': 'main.ok',
            'args': ['hello'], 'stdin': 'input.txt', 'env': {'CASE_VALUE': 'yes'},
            'budget': {'timeout_s': 17}, 'requires': {'fixtures': ['sample-files']},
        }
        (self.case / 'case.json').write_text(json.dumps(data))
        fixture = self.root / 'conformance' / 'fixtures' / 'sample-files'
        (fixture / 'tree' / 'nested').mkdir(parents=True)
        (fixture / 'fixture.json').write_text(json.dumps({
            'schema': 'ocs-fixture/2', 'kind': 'files', 'path': 'tree', 'target': 'fixture-root'
        }))
        (fixture / 'tree' / 'nested' / 'data.txt').write_text('fixture payload')
        self.item = {'case_dir': str(self.case)}

    def tearDown(self):
        self.tmp.cleanup()

    def test_copies_case_and_fixtures_and_returns_process_inputs(self):
        dest = self.root / 'scratch' / 'work'
        setup = oracle.prepare_case(self.item, self.root, dest)
        self.assertEqual(Path(setup['source']), dest / 'main.ok')
        self.assertEqual(Path(setup['cwd']), dest)
        self.assertEqual(setup['args'], ['hello'])
        self.assertEqual(setup['env'], {'CASE_VALUE': 'yes'})
        self.assertEqual(setup['timeout'], 17)
        self.assertEqual(Path(setup['stdin']).read_bytes(), b'input bytes')
        self.assertEqual((dest / 'support.ok').read_text(), 'support source')
        self.assertEqual((dest / 'fixture-root' / 'nested' / 'data.txt').read_text(), 'fixture payload')
        self.assertFalse((dest / 'case.json').exists())

    def test_service_fixture_is_reported_as_unsupported(self):
        fixture = self.root / 'conformance' / 'fixtures' / 'sample-files' / 'fixture.json'
        fixture.write_text(json.dumps({'schema': 'ocs-fixture/2', 'kind': 'service'}))
        with self.assertRaisesRegex(oracle.CaseSetupError, 'only ocs-fixture/1 files fixtures'):
            oracle.prepare_case(self.item, self.root, self.root / 'unsupported')

    def test_harness_is_reported_as_unsupported(self):
        path = self.case / 'case.json'
        data = json.loads(path.read_text())
        data['harness'] = True
        path.write_text(json.dumps(data))
        with self.assertRaisesRegex(oracle.CaseSetupError, 'harness setup'):
            oracle.prepare_case(self.item, self.root, self.root / 'unsupported')


if __name__ == '__main__':
    unittest.main()
