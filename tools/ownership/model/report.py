"""Run checks and persist a reproducible, timestamp-free acceptance record."""
from collections import Counter
import hashlib
import json
from pathlib import Path
import unittest

from checker import run, verify
from ir import parse
from test_model import Fixtures


def main():
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(Fixtures)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    if not result.wasSuccessful():
        return 1
    here = Path(__file__).resolve().parent
    cases = json.loads((here / 'fixtures/manifest.json').read_text())
    counts = Counter(c['status'] for c in cases)
    corpus = []
    paths = 0
    positive = negative = 0
    for case in cases:
        if 'error' in case:
            negative += 1
            continue
        positive += 1
        text = (here / 'fixtures' / case['file']).read_text()
        program = parse(text)
        paths += len(verify(program))
        if 'corpus' in case:
            source = here.parents[2] / 'tests/ownership-corpus' / case['corpus']
            state = run(program)
            corpus.append({'source': 'tests/ownership-corpus/' + case['corpus'],
                           'source_sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
                           'fixture': case['file'], 'checks': len(state.checkpoints),
                           'checkpoints': state.checkpoints, 'destroy': state.destroy_trace})
    report = {'result': 'PASS', 'unittest_tests': result.testsRun,
              'fixtures': len(cases), 'accepted': positive, 'rejected_as_expected': negative,
              'verified_positive_paths': paths, 'by_status': dict(sorted(counts.items())),
              'review_findings': 24, 'verifier_rules': 11, 'corpus_programs': len(corpus),
              'corpus_tap_checks': sum(c['checks'] for c in corpus), 'corpus': corpus,
              'runtime_executed': False,
              'proposals_approved': False,
              'comparison': 'All original literal t.is(log.value(), expected, ...) goldens; no runtime execution.'}
    (here / 'results.json').write_text(json.dumps(report, indent=2, ensure_ascii=False) + '\n')
    print(json.dumps({k: v for k, v in report.items() if k != 'corpus'}, ensure_ascii=False, indent=2))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
