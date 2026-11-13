"""Independent fixture expectations and corpus TAP goldens; no runtime invocation."""
import json
from pathlib import Path
import re
import sys
import unittest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from checker import run, verify
from ir import ModelError, parse
from machine import Machine


class Fixtures(unittest.TestCase):
    def test_catalog(self):
        catalog = json.loads((HERE / 'fixtures/manifest.json').read_text())
        names = [c['file'] for c in catalog]
        self.assertEqual(len(names), len(set(names)), 'duplicate catalog entries inflate coverage')
        self.assertEqual(set(names), {str(p.relative_to(HERE / 'fixtures'))
                                     for p in (HERE / 'fixtures').rglob('*.oir')})
        reviews, rules, corpus, statuses = set(), set(), set(), {}
        for case in catalog:
            with self.subTest(fixture=case['file']):
                source = (HERE / 'fixtures' / case['file']).read_text()
                program = parse(source)
                self.assertEqual(program.text(), parse(program.text()).text())
                reviews.update(case.get('review', []))
                rules.update(case.get('rules', []))
                statuses[case['status']] = statuses.get(case['status'], 0) + 1
                if 'error' in case:
                    with self.assertRaises(ModelError) as caught:
                        verify(program)
                    self.assertEqual(caught.exception.rule, case['error'])
                    self.assertIn(case['message'], caught.exception.message)
                    continue
                paths = verify(program)
                self.assertEqual(len(paths), case.get('paths', 1))
                state = run(program)
                self.assertEqual(state.destroy_trace, case['destroy'])
                self.assertEqual(state.log, case['log'])
                self.assertEqual(state.checkpoints, case.get('checkpoints', []))
                if 'corpus' in case:
                    path = HERE.parents[2] / 'tests/ownership-corpus' / case['corpus']
                    corpus.add(case['corpus'])
                    text = path.read_text()
                    # Read only literal expected log strings in the original TAP
                    # checks. Expected model traces are separately hand written.
                    goldens = [json.loads(s) for s in re.findall(
                        r't\.is\(log\.value\(\),\s*("(?:[^"\\]|\\.)*")', text)]
                    self.assertEqual(len(goldens), int(re.search(r'Tap\((\d+)\)', text)[1]))
                    self.assertEqual(state.checkpoints, goldens)
        self.assertEqual(reviews, set(range(1, 25)))
        self.assertEqual(rules, {f'B{i}' for i in range(1, 12)})
        self.assertGreaterEqual(len(corpus), 25)

    def test_every_rule_has_negative_fixture(self):
        catalog = json.loads((HERE / 'fixtures/manifest.json').read_text())
        self.assertEqual({c['error'] for c in catalog if 'error' in c}, {f'B{i}' for i in range(1, 12)})

    def test_counterexamples_have_individual_files(self):
        catalog = json.loads((HERE / 'fixtures/manifest.json').read_text())
        review_files = [c for c in catalog if c['file'].startswith('review/')]
        self.assertEqual({n for c in review_files for n in c['review']}, set(range(1, 25)))
        self.assertGreaterEqual(len(review_files), 24)

    def test_global_checker_detects_corrupted_holder_and_duplicate(self):
        machine = Machine()
        for instruction in parse('profile own\nscope.begin s\nnew a A\n').instructions:
            machine.execute(instruction)
        machine.bag('corrupt', 's')
        machine.bags['corrupt'].add(1)
        with self.assertRaisesRegex(ModelError, 'nonlinear obligation'):
            machine.invariants()
        machine.bags['corrupt'].clear()
        machine.identities[1].holder = 'corrupt'
        with self.assertRaisesRegex(ModelError, 'holder metadata'):
            machine.invariants()

    def test_destroy_locals_are_per_invocation_and_events_are_repeatable(self):
        program = parse('''profile own
scope.begin s
new a A
on.destroy a 'alias local self' 'use local' 'new temporary T'
new b B
on.destroy b 'alias local self' 'use local' 'new temporary T'
scope.end s
''')
        first, second = run(program), run(program)
        self.assertEqual(first.destroy_trace, list('BTAT'))
        self.assertEqual(first.events, second.events)
        begins = [e['id'] for e in first.events if e['event'] == 'destroy.begin']
        self.assertEqual(len(begins), len(set(begins)))

    def test_parser_refuses_unknown_ops_arity_and_successors(self):
        for text, code in [
            ('profile own\nsurprise x\n', 'IR'),
            ('profile own\nnew missing_label\n', 'IR'),
            ('profile own\njump missing\n', 'B6'),
            ('scope.begin s\n', 'B11'),
        ]:
            with self.subTest(text=text), self.assertRaises(ModelError) as caught:
                parse(text)
            self.assertEqual(caught.exception.rule, code)

    def test_checked_split_failure_path_preserves_input(self):
        text = (HERE / 'fixtures/valid/checked-split-normal-and-unwind.oir').read_text()
        # This is a failure injection, not a second independent golden.
        text = text.replace('invoke.split input a ok', 'invoke.split input a fail')
        state = run(parse(text))
        self.assertEqual(state.destroy_trace, ['B', 'A'])
        self.assertFalse(state.packets)

    def test_untaken_branch_is_still_verified(self):
        text = (HERE / 'fixtures/invalid/B6-error-only-on-untaken-branch.oir').read_text()
        self.assertEqual(run(parse(text)).destroy_trace, [])
        with self.assertRaisesRegex(ModelError, 'consumed packet'):
            verify(parse(text))

    def test_nested_defer_failure_closes_locals_and_continues_other_defers(self):
        state = run(parse('''profile own
scope.begin s
new a A
defer s emit d
defer.block s 'scope.begin defer_scope' 'new temporary T' 'fail typed Defer' 'emit wrong'
scope.end s
assert.failure Defer
'''))
        self.assertEqual(state.destroy_trace, ['T', 'A'])
        self.assertEqual(state.log, 'TdA')

    def test_readme_example_executes(self):
        text = (HERE / 'README.md').read_text()
        example = re.search(r'```text\n(profile own\n.*?)\n```', text, re.S)[1]
        self.assertEqual(run(parse(example)).log, 'BBA')

    def test_staging_and_carrier_publication_end_fresh_call_admission(self):
        state = run(parse('''profile own
summary read borrow direct
scope.begin s
new a A
stage s item a
call.enter staged_read read a
call.exit staged_read -
use a
new b B
failure f typed Internal
scope.split s b packet
carrier f packet s
failure.view view f
call.enter carrier_read read view
call.exit carrier_read -
use view
check ''
scope.end s
'''))
        self.assertEqual(state.destroy_trace, ['B', 'A'])


if __name__ == '__main__':
    unittest.main()
