"""CPU guard parsing and preserved evidence invariants; no benchmarks/builds."""
import unittest

from cr_pgo_recheck import assess
from cr_pgo_reduce import verify


class CpuGuardTests(unittest.TestCase):
    def block(self, idle: float = 95, cpu: float = 1, pid=123, command='competitor'):
        return (f'CPU usage: 1% user, 4% sys, {idle}% idle\n'
                f'{pid} {command} {cpu} 00:01.00 2\n')

    def test_idle_and_process_thresholds(self):
        self.assertFalse(assess(self.block(90, 49.9), 90, set())['busy'])
        self.assertTrue(assess(self.block(89.99, 1), 90, set())['busy'])
        self.assertTrue(assess(self.block(99, 50), 85, set())['busy'])

    def test_owned_and_expected_binaries(self):
        self.assertFalse(assess(self.block(95, 99), 85, {123})['busy'])
        for command in ('baseline', 'candidate', 'top'):
            self.assertFalse(assess(self.block(95, 99, command=command), 85, set())['busy'])
        self.assertTrue(assess(self.block(84, 99, command='baseline'), 85, set())['busy'])

    def test_missing_cpu_line_fails_closed(self):
        self.assertTrue(assess('partial top output', 85, set())['busy'])

    def test_full_evidence_replay(self):
        summary = verify()
        self.assertEqual(len(summary['comparisons']), 10)
        self.assertEqual(len(summary['cpu_windows']), 3)
        for campaign in summary['cr_campaigns'].values():
            self.assertEqual(campaign['measured_samples'], 480)
            self.assertTrue(campaign['overlapping_cpu_windows'])


if __name__ == '__main__':
    unittest.main()
