#!/usr/bin/env python3
"""Synthetic analyzer invariants. No private corpus needed."""
from collections import Counter
import json
import unittest

from ao_kernel_survey import aggregate, operations, static_path
import ast


class OperationsTest(unittest.TestCase):
    def test_static_paths_and_siblings(self):
        result = operations('a = p["data"]["a"]\nb = p["data"]["b"]\nprint(p["data"]["a"])')
        self.assertEqual(result['same_receiver_distinct_literal_siblings'], 1)
        self.assertEqual(result['nested_literal_key_read'], 3)
        self.assertEqual(result['repeated_literal_path'], 2)
        self.assertEqual(result['literal_key_read'], 6)
        self.assertIsNone(static_path(ast.parse('p[key]', mode='eval').body))
        self.assertEqual(operations('p["a"] = 1')['literal_key_read'], 0)

    def test_json_call_spelling_is_not_type_inference(self):
        result = operations('x = json.loads(s)\nprint(json.dumps(x["data"]))\nx = codec.loads(s)')
        self.assertEqual(result['json_decode_call'], 1)
        self.assertEqual(result['json_encode_call'], 1)
        self.assertEqual(result['literal_key_read_passed_to_json_encode'], 1)
        self.assertNotIn('typed_json', result)

    def test_string_membership_direction_and_chains(self):
        result = operations('print("needle" in s, s in "haystack", k in d)\nprint(s.startswith("x"), "prefix".startswith(s))\nprint("\\n".join(s.splitlines()))')
        self.assertEqual(result['membership_untyped'], 3)
        self.assertEqual(result['literal_needle_membership_untyped_haystack'], 1)
        self.assertEqual(result['membership_literal_string_haystack'], 1)
        self.assertEqual(result['method_startswith_untyped'], 2)
        self.assertEqual(result['method_startswith_literal_receiver'], 1)
        self.assertEqual(result['method_join_literal_receiver'], 1)
        self.assertEqual(result['method_splitlines_untyped'], 1)

    def test_comprehensions_and_reductions_remain_untyped(self):
        result = operations('a = [r["n"] for r in rows if r["ok"]]\nb = all(r.ok for r in rows)\nc = sum(r.n for r in rows)\nd = next((r for r in rows if r.ok), None)\ne = len([r for r in rows if r.ok])')
        self.assertEqual(result['filtered_comprehension'], 3)
        self.assertEqual(result['field_projection_comprehension'], 3)
        self.assertEqual(result['builtin_sum_generator_untyped'], 1)
        self.assertEqual(result['sum_generator_field_term_untyped'], 1)
        self.assertEqual(result['builtin_all_generator_untyped'], 1)
        self.assertEqual(result['builtin_next_generator_untyped'], 1)
        self.assertEqual(result['len_listcomp_untyped'], 1)
        self.assertEqual(result['single_sync_comprehension'], 5)
        terms = operations('sum(x > 0 for x in rows)\nsum(1 for x in rows if x.ok)\nsum(x * 2 for x in rows)')
        self.assertEqual(terms['sum_generator_predicate_syntax'], 1)
        self.assertEqual(terms['sum_generator_integer_literal_term'], 1)
        self.assertEqual(terms['sum_generator_other_term_untyped'], 1)

    def test_loop_add_is_not_checked_integer_proof(self):
        result = operations('for r in rows:\n    if r.ok:\n        total += r.n\n    def f():\n        text += "suffix"\n')
        self.assertEqual(result['loop_augadd_untyped'], 1)
        self.assertNotIn('checked_i64_sum', result)

    def test_overlapping_syntax_is_not_dynamic_work(self):
        result = operations('if False:\n    for x in rows:\n        y = p["k"]\n        print(p["k"])')
        self.assertEqual(result['literal_key_read'], 2)
        self.assertEqual(result['repeated_literal_path'], 1)
        self.assertEqual(aggregate([result, result])['presence']['repeated_literal_path'], 2)
        self.assertEqual(aggregate([result, result])['occurrences']['literal_key_read'], 4)

    def test_values_and_identifiers_never_exported_or_executed(self):
        result = operations('private_identifier["PRIVATE_SENTINEL"]\nraise RuntimeError("not executed")')
        encoded = json.dumps(result)
        self.assertNotIn('PRIVATE_SENTINEL', encoded)
        self.assertNotIn('private_identifier', encoded)
        self.assertEqual(result, Counter(literal_key_read=1))

    def test_unknown_calls_not_reported_as_supported_kernels(self):
        result = operations('obj.sum(xs)\nobj.any(xs)\nfind(xs)\nfilter = other\nfilter(f, xs)')
        self.assertEqual(result, Counter(builtin_filter_unresolved=1))
        with self.assertRaises(SyntaxError):
            operations('this is not valid Python !')


if __name__ == '__main__':
    unittest.main()
