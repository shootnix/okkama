import unittest

from foundation_audit import contract_review, historical_result, index_unique


class ContractReviewTests(unittest.TestCase):
    def test_historical_pass_requires_matching_inputs_and_never_becomes_current_pass(self):
        self.assertEqual(historical_result({'status': 'PASS'}, True), 'historical_pass')
        self.assertEqual(historical_result({'status': 'PASS'}, False), 'source_mismatch')
        for status in ('FAIL', 'BLOCKED', 'NOT_RUN', None):
            self.assertEqual(historical_result({'status': status}, True), 'historical_nonpass')
        self.assertEqual(historical_result(None, True), 'absent')

    def test_deferred_and_informative_are_not_missing_mandatory_coverage(self):
        inventory = [dict(id='decimal', cases=[]), dict(id='substrate', cases=[])]
        definitions = [dict(id='decimal', status='deferred', discrepancies=['D-1']),
                       dict(id='substrate', status='informative')]
        rows = contract_review(inventory, definitions)
        self.assertEqual([r['review_action'] for r in rows], ['tracked_not_mandatory'] * 2)
        self.assertEqual(rows[0]['discrepancies'], ['D-1'])

    def test_a_case_link_does_not_prove_all_required_aspects(self):
        rows = contract_review([dict(id='x', cases=['one'])], [
            dict(id='x', status='mandatory', aspects={'normal': 'required', 'invalid': 'required'})])
        self.assertEqual(rows[0]['review_action'], 'review_required_aspects')
        self.assertEqual(rows[0]['aspects']['invalid'], 'required')

    def test_missing_unresolved_and_unknown_remain_distinct(self):
        rows = contract_review([dict(id=i, cases=[]) for i in ('x', 'y', 'z')],
                               [dict(id='x', status='mandatory'), dict(id='y', status='unresolved')])
        self.assertEqual([r['review_action'] for r in rows], [
            'missing_case_mapping', 'unresolved_blocks_release', 'unknown_status_requires_review'])

    def test_duplicate_evidence_is_rejected(self):
        with self.assertRaises(ValueError):
            index_unique([dict(path='x'), dict(path='x')], 'path')
        with self.assertRaises(ValueError):
            contract_review([], [dict(id='x'), dict(id='x')])


if __name__ == '__main__':
    unittest.main()
