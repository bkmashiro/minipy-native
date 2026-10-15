"""Contract tests for the small workbench, no builds or timing needed."""
import copy
import gzip
import json
from pathlib import Path
import tempfile
import unittest

import perf_workbench as pw


def rows():
    return [{'kind': 'setup', 'ns': 999999, 'diagnostic': False},
            {'kind': 'request', 'sample': 0, 'ns': 888888},
            {'kind': 'request', 'sample': 1, 'ns': 10},
            {'kind': 'request', 'sample': 2, 'ns': 20}]


def campaign():
    records = [{'kind': 'manifest', 'rounds': 1, 'reps': 2, 'diagnostic': False,
                'aa': True, 'cells': [['tape', 'scalar']]}]
    for slot, arm in enumerate('ABBA'):
        records.append({'kind': 'process', 'round': 0, 'slot': slot, 'arm': arm,
                        'backend': 'tape', 'case': 'scalar', 'ok': True,
                        'error': None, 'exit_code': 0, 'rows': rows()})
    records.append({'kind': 'end', 'failures': 0})
    return records


class Reduction(unittest.TestCase):
    def test_input_cases_are_opt_in(self):
        self.assertEqual(pw.DEFAULT_CASES, ('scalar', 'quote', 'ready', 'loopback'))
        self.assertEqual(pw.CASES, pw.DEFAULT_CASES + (
            'scalar_inputs', 'ready_inputs', 'plain_sink', 'escaped_sink',
            'joined_sink', 'fstring_sink', 'escaped_join', 'madd_short', 'madd_loop',
            'detach', 'detach_live'))

    def reduce(self, records):
        with tempfile.TemporaryDirectory(dir=pw.SCRATCH) as directory:
            path = Path(directory) / 'raw.jsonl.gz'
            with gzip.open(path, 'wt') as f:
                for record in records:
                    f.write(json.dumps(record) + '\n')
            return pw.reduce(path)

    def test_exclude_setup_and_warmup(self):
        result = self.reduce(campaign())
        self.assertTrue(result['complete'])
        self.assertEqual(result['measured_samples'], 8)
        self.assertEqual(result['startup_median_ns'], 999999)
        self.assertEqual(result['cells'][0]['medians_ns'], {'A': 15, 'B': 15})
        self.assertEqual(result['cells'][0]['b_over_a'], 1)

    def test_missing_process_no_delta(self):
        data = campaign()
        del data[2]
        result = self.reduce(data)
        self.assertFalse(result['complete'])
        self.assertEqual(len(result['missing_processes']), 1)
        self.assertNotIn('b_over_a', result['cells'][0])

    def test_failure_partial_rows_not_timed(self):
        data = campaign()
        data[2].update(ok=False, error='oracle panic', exit_code=101)
        data[-1]['failures'] = 1
        result = self.reduce(data)
        self.assertFalse(result['complete'])
        self.assertEqual(result['measured_samples'], 6)
        self.assertEqual(len(result['failures']), 1)
        self.assertNotIn('b_over_a', result['cells'][0])

    def test_duplicate_process_rejected(self):
        data = campaign()
        data.insert(2, copy.deepcopy(data[1]))
        with self.assertRaises(ValueError):
            self.reduce(data)

    def test_missing_or_duplicate_sample_rejected(self):
        for sample in [0, 3]:
            data = campaign()
            data[1]['rows'][2]['sample'] = sample
            with self.assertRaises(ValueError):
                self.reduce(data)

    def test_missing_end_incomplete(self):
        result = self.reduce(campaign()[:-1])
        self.assertFalse(result['complete'])
        self.assertNotIn('b_over_a', result['cells'][0])

    def test_diagnostic_identity_rejected(self):
        data = campaign()
        data[1]['rows'][0]['diagnostic'] = True
        with self.assertRaises(ValueError):
            self.reduce(data)

    def test_invalid_timing_rejected(self):
        for value in [0, -1, True, None, 1.5]:
            data = campaign()
            data[1]['rows'][2]['ns'] = value
            with self.assertRaises(ValueError):
                self.reduce(data)


if __name__ == '__main__':
    unittest.main()
