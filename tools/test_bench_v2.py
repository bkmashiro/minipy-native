"""Deterministic arithmetic/accounting tests, not performance evidence."""
import unittest
from unittest.mock import patch
from pathlib import Path
from summarize_bench_v2 import fit, percentile, summarize
from run_bench_v2 import identity, plan


class SummaryTests(unittest.TestCase):
    def test_missing_guest_toolchain_is_unavailable_metadata(self):
        with patch('run_bench_v2.subprocess.run', side_effect=FileNotFoundError):
            result = identity(Path('.'))
        self.assertIsNone(result['toolchain'])
        self.assertIsNone(result['source_commit'])

    def test_plan_is_small_and_classified(self):
        self.assertEqual(len(plan('smoke')), 17)
        self.assertEqual(len(plan('campaign')), 39)
        result = summarize([], [{'status': 'completed'}, {'status': 'failed'}, {'status': 'timeout'}])
        self.assertEqual(result['commands'], {'scheduled': 3, 'completed': 1, 'failed': 1, 'timeout': 1})

    def test_fit_and_missing(self):
        result = fit([(0, 11), (1, 17), (8, 59)])
        assert result is not None
        self.assertEqual(result['slope'], 6)
        self.assertIsNone(fit([(0, 2)]))
        self.assertEqual(percentile([1, 2, 100], .99), 100)
        self.assertIsNone(percentile([], .99))

    def test_break_even_is_model_not_new_observation(self):
        rows = []
        for engine, cold, warm in [('native', 100, 2), ('monty', 20, 12)]:
            for life, total in [('cold_source_to_result', cold), ('prepared_request', warm)]:
                rows.append({'kind': 'latency', 'arm': 'fixture', 'engine': engine, 'case': 'case', 'rows': 1, 'lifecycle': life, 'chunk': 256, 'delay_us': 0, 'status': 'completed', 'total_wall_ns': total, 'total_cpu_ns': 0, 'phases': {'compile': 0}, 'independent_run': 0})
        result = summarize(rows, [])
        self.assertEqual(result['break_even_model'][0]['modeled_reuse_requests'], 9)
        self.assertEqual(result['latency'][0]['independent_runs'], 1)
        self.assertEqual(result['latency'][0]['cpu_ns'], 0)

    def test_missing_request_is_an_error(self):
        row = {'kind': 'concurrent', 'arm': 'fixture', 'engine': 'native', 'case': 'case', 'rows': 1, 'concurrency': 1, 'chunk': 256, 'delay_us': 0, 'mixed': True, 'requests': 2, 'completed': 2, 'failed': 0, 'command_index': 0}
        with self.assertRaisesRegex(AssertionError, 'missing request'):
            summarize([row], [])

    def test_burst_and_cold_arrival_are_never_pooled(self):
        rows = []
        for index, life, interval in [(0, 'prepared_shared_code_bounded_admission', None), (1, 'cold_fixed_arrival_bounded_admission', 1000)]:
            rows.append({'kind': 'concurrent', 'arm': 'fixture', 'engine': 'native', 'case': 'case', 'rows': 1, 'concurrency': 1, 'chunk': 256, 'delay_us': 0, 'mixed': True, 'requests': 1, 'completed': 1, 'failed': 0, 'command_index': index, 'lifecycle': life, 'arrival_us': interval, 'total_wall_ns': 12, 'total_cpu_ns': 8, 'throughput_per_second': 1})
            rows.append({'kind': 'request', 'command_index': index, 'engine': 'native', 'request': 0, 'status': 'completed', 'class': 'short', 'queue_ns': 4, 'service_ns': 8, 'latency_ns': 12})
        result = summarize(rows, [])
        self.assertEqual(len(result['concurrent']), 2)
        self.assertEqual({r['arrival_us'] for r in result['concurrent']}, {0, 1000})


if __name__ == '__main__':
    unittest.main()
