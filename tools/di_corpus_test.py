#!/usr/bin/env python3
"""No Cargo/model/corpus campaign: inventory and no-partial-binding invariants."""
import unittest
from typing import Any
import di_corpus as corpus


class InventoryTests(unittest.TestCase):
    def test_uniform_output_policy_validation(self):
        import argparse
        for text in ['0', '7', '262144', '16777216']:
            self.assertEqual(corpus.stdout_bytes(text), int(text))
        for text in ['-1', '+1', '1.0', 'true', '16777217', '', '１２', ' 1']:
            with self.assertRaises(argparse.ArgumentTypeError):
                corpus.stdout_bytes(text)

    def test_whole_source_denominator_keeps_unsupported_tail(self):
        sites = corpus.inventory("x = tools.fetch(1)\nprint(x['a']['b'])\ny = [tools.fetch(2)]")
        self.assertEqual(sites, {'tool_call_sites': 2, 'data_selector_sites': 2})
        row = corpus.decorate({'whole_source_admitted': False}, {
            'source': "x = tools.fetch(1)\ny = [1]", 'contract_complete': True})
        self.assertEqual(row['static_binding']['source_sites']['tool_call_sites'], 1)
        self.assertIsNone(row['static_binding']['whole_program_bound_sites'])

    def test_incomplete_contract_does_not_become_binding_coverage(self):
        for contract in [False, None]:
            row = corpus.decorate({'whole_source_admitted': True}, {
                'source': 'print(1)', 'contract_complete': contract})
            self.assertIsNone(row['static_binding']['whole_program_bound_sites'])
        row = corpus.decorate({'whole_source_admitted': True}, {
            'source': "d = tools.fetch()\nprint(d.get('name','x'))", 'contract_complete': True})
        self.assertEqual(row['static_binding']['whole_program_bound_sites'],
                         {'tool_call_sites': 1, 'data_selector_sites': 1})

    def test_correctness_requires_complete_contract_oracle_and_expected(self):
        expected = {'stdout': '1\n', 'status': 'ok', 'calls': []}
        for contract in [False, None, True, 1]:
            for oracle in [False, None, True, 1]:
                for supplied in [False, True]:
                    case: dict[str, Any] = {'source': 'print(1)'}
                    if contract is not None:
                        case['contract_complete'] = contract
                    if oracle is not None:
                        case['oracle_complete'] = oracle
                    if supplied:
                        case['expected'] = expected
                    eligible = contract is True and oracle is True and supplied
                    for equality in [False, True]:
                        with self.subTest(case=case, equality=equality):
                            row = corpus.decorate({'correct': equality}, case)
                            self.assertIs(row['correct'], equality if eligible else None)
        case = {'source': 'print(1)', 'contract_complete': True,
                'oracle_complete': True, 'expected': None}
        self.assertIsNone(corpus.decorate({'correct': True}, case)['correct'])

    def test_syntax_has_unknown_denominators_and_calls_are_not_field_sites(self):
        self.assertIsNone(corpus.inventory('print('))
        self.assertEqual(corpus.inventory("d = tools.fetch()\nprint(d['s'].startswith('a'))"),
                         {'tool_call_sites': 1, 'data_selector_sites': 1})


if __name__ == '__main__':
    unittest.main()
