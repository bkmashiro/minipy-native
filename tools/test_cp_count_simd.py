"""Named invariants for CP's coarse CPU guard and restored evidence replay."""
import contextlib
import io
import unittest

from cp_count_simd import assess, replay


class CpCountTests(unittest.TestCase):
    def test_guard_fails_closed_without_cpu_line(self):
        self.assertTrue(assess('partial snapshot\n', 85)['busy'])

    def test_guard_idle_threshold(self):
        self.assertFalse(assess('CPU usage: 1% user, 9% sys, 90% idle\n', 90)['busy'])
        self.assertTrue(assess('CPU usage: 1% user, 9% sys, 89.99% idle\n', 90)['busy'])

    def test_guard_competitor_threshold(self):
        prefix = 'CPU usage: 1% user, 1% sys, 98% idle\n'
        self.assertFalse(assess(prefix + '999999 unrelated 49.9\n', 85)['busy'])
        self.assertTrue(assess(prefix + '999999 unrelated 50.0\n', 85)['busy'])
        self.assertFalse(assess(prefix + '999999 cp-simd 100.0\n', 85)['busy'])

    def test_evidence_replay_from_restored_scalar_source(self):
        with contextlib.redirect_stdout(io.StringIO()):
            replay()


if __name__ == '__main__':
    unittest.main()
