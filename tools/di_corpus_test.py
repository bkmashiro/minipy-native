#!/usr/bin/env python3
"""No Cargo/model/corpus campaign: inventory and no-partial-binding invariants."""
import unittest
import di_corpus as corpus


class InventoryTests(unittest.TestCase):
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

    def test_syntax_has_unknown_denominators_and_calls_are_not_field_sites(self):
        self.assertIsNone(corpus.inventory('print('))
        self.assertEqual(corpus.inventory("d = tools.fetch()\nprint(d['s'].startswith('a'))"),
                         {'tool_call_sites': 1, 'data_selector_sites': 1})


if __name__ == '__main__':
    unittest.main()
