"""The fixed acceptance entry must not count empty, skipped or filtered runs."""
import unittest

from acceptance import listed_tests, passed_tests


class AcceptanceOutput(unittest.TestCase):
    def test_inventory_requires_sentinel_and_exact_unique_count(self):
        self.assertEqual(listed_tests('contract: test\n\n1 test, 0 benchmarks\n', 'contract'),
                         ['contract'])
        for text in ['0 tests, 0 benchmarks\n',
                     'other: test\n\n1 test, 0 benchmarks\n',
                     'contract: test\ncontract: test\n\n2 tests, 0 benchmarks\n',
                     'contract: test\n\n2 tests, 0 benchmarks\n']:
            with self.subTest(text=text), self.assertRaises(ValueError):
                listed_tests(text, 'contract')

    def test_result_requires_all_tests_and_no_skips(self):
        good = 'test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n'
        passed_tests(good, 2)
        bad = ['', good + good, good.replace('2 passed', '0 passed'),
               good.replace('0 failed', '1 failed'),
               good.replace('0 ignored', '1 ignored'),
               good.replace('0 measured', '1 measured'),
               good.replace('0 filtered', '1 filtered'),
               good.replace('ok.', 'FAILED.')]
        for text in bad:
            with self.subTest(text=text), self.assertRaises(ValueError):
                passed_tests(text, 2)


if __name__ == '__main__':
    unittest.main()
