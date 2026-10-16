"""CU policy/identity checks without compilation or performance runs."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import cu_pgo as cu
from cr_pgo_recheck import assess


class Policy(unittest.TestCase):
    def test_frozen_split_and_weights(self):
        self.assertFalse(set(cu.TRAIN) & set(cu.HOLDOUT))
        self.assertEqual(tuple(cu.WEIGHTS), ('balanced', 'strings'))
        self.assertEqual(sum(cu.WEIGHTS['balanced'].values()), 144)
        self.assertEqual(sum(cu.WEIGHTS['strings'].values()), 360)
        self.assertEqual(len(set(cu.CASES)), 11)
        self.assertTrue(set(cu.HOLDOUT) <= set(cu.CASES))
        self.assertFalse({'multi_branch', 'multi_overflow', 'multi_permission'} & set(cu.TRAIN))

    def test_cpu_guard(self):
        self.assertFalse(assess('CPU usage: 99.0% idle\n12 candidate 75.0 00:01', 85, set())['busy'])
        self.assertTrue(assess('CPU usage: 84.9% idle', 85, set())['busy'])
        self.assertTrue(assess('CPU usage: 99.0% idle\n12 unrelated 50.0 00:01', 85, set())['busy'])
        self.assertTrue(assess('missing CPU row', 85, set())['busy'])

    def test_stale_source_profile_and_toolchain_rejected(self):
        with tempfile.TemporaryDirectory(dir=cu.SCRATCH) as directory:
            folder = Path(directory)
            (folder / 'plan.json').write_text(json.dumps({'sources': {'src/x.rs': 'current'}}))
            (folder / 'profiles.json').write_text(json.dumps({name: {'profile_sha256': 'profile'} for name in cu.WEIGHTS}))
            (folder / 'baseline-identity.json').write_text(json.dumps({'rust': 'fixed toolchain\n'}))
            with patch.object(cu, 'OUT', folder), patch.object(cu.cn, 'sources', return_value={'src/x.rs': 'current'}), \
                 patch.object(cu, 'sha', return_value='profile') as digest, \
                 patch.object(cu.subprocess, 'check_output', return_value='fixed toolchain\n') as compiler:
                cu.verify_profiles()
                with patch.object(cu.cn, 'sources', return_value={'src/x.rs': 'stale'}):
                    with self.assertRaisesRegex(AssertionError, 'retrain'):
                        cu.verify_profiles()
                digest.return_value = 'different profile'
                with self.assertRaises(AssertionError):
                    cu.verify_profiles()
                digest.return_value = 'profile'
                compiler.return_value = 'different toolchain\n'
                with self.assertRaises(AssertionError):
                    cu.verify_profiles()


if __name__ == '__main__':
    unittest.main()
