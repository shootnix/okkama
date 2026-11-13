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
                    # Read every literal expected string in the original TAP checks,
                    # including scalar results. Model goldens are hand written.
                    goldens = [json.loads(s) for s in re.findall(
                        r't\.is\([^,\n]+,\s*("(?:[^"\\]|\\.)*")', text)]
                    self.assertEqual(len(goldens), int(re.search(r'Tap\((\d+)\)', text)[1]))
                    expected = list(goldens)
                    for difference in case.get('corpus_differences', []):
                        index = difference['index']
                        self.assertEqual(goldens[index], difference['source'])
                        self.assertNotEqual(difference['source'], difference['model'])
                        self.assertTrue(difference['reason'])
                        expected[index] = difference['model']
                    self.assertEqual(state.checkpoints, expected)
        self.assertEqual(reviews, set(range(1, 25)))
        self.assertEqual(rules, {f'B{i}' for i in range(1, 12)})
        self.assertGreaterEqual(len(corpus), 40)
        self.assertFalse({'proposed-581', 'unresolved-fake'} & statuses.keys())

    def test_draft3_answers_have_positive_and_negative_witnesses(self):
        catalog = json.loads((HERE / 'fixtures/manifest.json').read_text())
        for answer in range(1, 10):
            cases = [c for c in catalog if answer in c.get('answers', [])]
            self.assertTrue(any('error' not in c for c in cases), answer)
            self.assertTrue(any('error' in c for c in cases), answer)

    def test_service_schedules_are_inputs_and_preserve_partial_orders(self):
        text = (HERE / 'fixtures/valid/M9-explicit-service-schedule.oir').read_text()
        first = run(parse(text))
        second = run(parse(text.replace('service.schedule s b a', 'service.schedule s a b')
                           .replace('check BYAX', 'check AXBY')))
        self.assertEqual(first.destroy_trace, list('BYAXO'))
        self.assertEqual(second.destroy_trace, list('AXBYO'))
        self.assertEqual(first.events, run(parse(text)).events)
        for state, order in [(first, ['b', 'a']), (second, ['a', 'b'])]:
            self.assertEqual([e['service'] for e in state.events
                              if e['event'] == 'service.choose' and e['scope'] == 's'], order)
            outer = next(i for i, e in enumerate(state.events)
                         if e['event'] == 'destroy' and e['label'] == 'O')
            settled = next(i for i, e in enumerate(state.events)
                           if e['event'] == 'service.end' and e['scope'] == 's')
            self.assertLess(settled, outer)

    def test_deferred_cancellation_has_separate_abort_boundary(self):
        state = run(parse((HERE / 'fixtures/valid/final-cleanup-cancel-aborts-result.oir').read_text()))
        self.assertEqual(state.destroy_trace, ['A', 'R'])
        aborts = [e for e in state.events if e['event'] == 'exit.abort']
        self.assertEqual([e['boundary'] for e in aborts], ['abort'])
        before = run(parse((HERE / 'fixtures/valid/M8-defer-failure-returns-reserve.oir').read_text()))
        self.assertEqual(before.destroy_trace, ['R', 'A'])
        self.assertEqual([e['boundary'] for e in before.events if e['event'] == 'exit.abort'], ['original-plan'])

    def test_partition_preserves_full_deps_and_retention_selection(self):
        state = run(parse((HERE / 'fixtures/valid/M4-retention-and-external-deps.oir').read_text()))
        self.assertEqual(state.values['both'].deps, frozenset({1, 2}))
        self.assertEqual(state.values['view'].ids, frozenset({2}))
        self.assertEqual(state.values['external_deps'].deps, frozenset({1}))

    def test_rehome_increments_epoch_once_and_noop_does_not(self):
        state = run(parse((HERE / 'fixtures/valid/M5-cache-owner-rehome.oir').read_text()))
        before = state.snapshots['before'][1]
        after = state.snapshots['after'][1]
        self.assertEqual(after[2], before[2] + 1)
        self.assertEqual(after[1], before[1])
        self.assertNotEqual(after[0], before[0])
        state = run(parse('profile own\nscope.begin s\nnew a A\ndynamic.refresh snap a\n'
                          'scope.transfer s s a\nsnapshot.check snap\nscope.end s\n'))
        self.assertEqual(state.identities[1].epoch, 0)

    def test_cached_failure_graph_has_one_packet_and_same_carrier(self):
        state = run(parse((HERE / 'fixtures/valid/M7-shared-failure-cache.oir').read_text()))
        self.assertIs(state.failures['first'], state.failures['second'])
        self.assertEqual(state.values['v'].deps, state.values['w'].deps)
        self.assertEqual(state.destroy_trace, ['A'])

    def test_all_futures_primary_by_schedule_secondaries_by_positions(self):
        state = run(parse("""profile own
scope.begin s
future a fail A bound
future b fail B bound
future c fail C bound
future.settle c
future.settle b
future.settle a
collect.all result a b c a
assert.failure C A B
collect.all again a b c a
assert.failure C A B
scope.end s
"""))
        self.assertEqual(state.futures['a'].starts, 1)
        self.assertEqual(len(state.futures['c'].failure.secondaries), 2)

    def test_completions_already_settled_inputs_follow_positions(self):
        state = run(parse("""profile own
scope.begin s
future a new A bound
future b new B bound
future.settle b
future.settle a
completions stream a b a
completion.next first stream
completion.next second stream
completion.next third stream
scope.end s
"""))
        self.assertEqual([state.values[n].futures for n in ('first', 'second', 'third')],
                         [frozenset({'a'}), frozenset({'b'}), frozenset({'a'})])
        self.assertEqual(state.destroy_trace, ['A', 'B'])

    def test_service_failure_continues_settle_and_race_cleans_all_losers(self):
        for finish in ('scope.end s', 'race.winner result winner a b\nscope.end s'):
            state = run(parse("""profile own
scope.begin s async
future winner new W bound
future a void - bound
future.body a 'new localA A' 'on.destroy localA "fail typed Cleanup"' suspend
future b void - bound
future.body b 'new localB B' suspend
future.start a
future.start b
""" + finish))
            self.assertEqual(state.destroy_trace[:2], ['A', 'B'])
            self.assertEqual(state.futures['b'].state, 'settled')
            self.assertEqual(state.scopes['s'].phase, 'closed')
            if finish.startswith('scope.end'):
                self.assertEqual(state.last_failure.name, 'Cleanup')
            else:
                self.assertNotIn('result', state.values)

    def test_rehome_plain_completed_future_does_not_retain_dead_inputs(self):
        state = run(parse("""profile own
scope.begin outer
scope.begin inner
new a A
future f literal done bound a
await result f
check A
cache.rehome f outer
scope.end inner
await again f
check.value again done
scope.end outer
"""))
        self.assertEqual(state.destroy_trace, ['A'])

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
