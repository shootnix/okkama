import unittest
from coverage_check import compare_registry, index, validate_capabilities


class CoverageTests(unittest.TestCase):
    def test_native_id_and_signature_drift_is_visible(self):
        old = [dict(symbol='one', name='One', id=1, signature='() -> int'),
               dict(symbol='gone', name='Gone', id=2, signature='() -> int')]
        new = [dict(symbol='one', name='One', id=3, signature='() -> bool'),
               dict(symbol='new', name='New', id=4, signature='() -> int')]
        result = compare_registry(old, new, ['name', 'id', 'signature'])
        self.assertEqual(result['added'], ['new'])
        self.assertEqual(result['removed'], ['gone'])
        self.assertEqual(result['changed'], [dict(symbol='one', fields=['id', 'signature'])])

    def test_duplicate_declarations_do_not_disappear_in_a_set(self):
        with self.assertRaises(ValueError):
            index([dict(symbol='x'), dict(symbol='x')], 'symbol')
        with self.assertRaises(ValueError):
            index([dict(id=1), dict(id=1)], 'id')

    def test_unsupported_is_a_valid_inventory_result_not_semantic_success(self):
        self.assertEqual(validate_capabilities([dict(symbol='x')], [dict(symbol='x', supported=False)]),
                         {'x': dict(symbol='x', supported=False)})

    def test_invalid_missing_duplicate_and_unknown_capabilities_are_rejected(self):
        for entries in [[], [dict(symbol='other', supported=True)],
                        [dict(symbol='x', supported='false')],
                        [dict(symbol='x', supported=1)],
                        [dict(symbol='x', supported=True), dict(symbol='x', supported=False)]]:
            with self.subTest(entries=entries):
                with self.assertRaises(ValueError):
                    validate_capabilities([dict(symbol='x')], entries)


if __name__ == '__main__':
    unittest.main()
