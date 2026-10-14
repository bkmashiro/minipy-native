"""Deterministic AD plan/accounting tests. Real execution is separate WSL smoke."""
import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('campaign', Path(__file__).with_name('ad_campaign.py'))
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)


class Contract(unittest.TestCase):
    def test_plan(self):
        work = c.schedule(Path('/home/na/benchmarks/minipy-s'))
        self.assertEqual(len(work), 300)
        self.assertEqual(sum(i['expected'] for i in work), 11769)
        self.assertEqual({i['arm'] for i in work}, set(c.ARMS))
        self.assertEqual(len([i for i in work if i['kind'] == 'speed']), 180)
        self.assertEqual(len([i for i in work if i['kind'] == 'mixed']), 48)
        self.assertEqual(len([i for i in work if i['kind'] == 'hold']), 36)
        self.assertEqual(len([i for i in work if i['kind'] == 'first-request']), 36)
        self.assertEqual({i['rows'] for i in work if i['kind'] == 'speed' and i['case'] == 'weighted'}, {100,300,1000,3000,10000,30000,100000})

    def first_fixture(self):
        item = next(i for i in c.schedule(Path('/x')) if i['kind'] == 'first-request')
        common = dict(cohort=c.COHORT, command_key=item['key'], round=item['round'])
        rows = [dict(common, kind='initialization'), dict(common, kind='first-request', completed=True,
                  stdout=c.oracle(item['case'], item['rows']), wall_ns=15,
                  **dict(zip(c.PHASES, [1,2,3,4,5])))]
        return item, rows

    def test_first_additivity_and_missing(self):
        item, rows = self.first_fixture()
        c.validate(rows, item)
        bad = copy.deepcopy(rows); bad[1]['execute_ns'] += 1
        with self.assertRaises(AssertionError): c.validate(bad, item)
        with self.assertRaises(AssertionError): c.validate(rows[:-1], item)
        bad = copy.deepcopy(rows); bad[1]['completed'] = False
        with self.assertRaises(AssertionError): c.validate(bad, item)

    def test_wrong_oracle_rejected(self):
        item, rows = self.first_fixture()
        rows[1]['stdout'] = 'wrong'
        with self.assertRaises(AssertionError): c.validate(rows, item)

    def test_old_cohort_rejected(self):
        item, rows = self.first_fixture()
        rows[0]['cohort'] = 'w-expanded-tool-response'
        with self.assertRaises(AssertionError): c.validate(rows, item)

    def test_output_independent_oracle(self):
        self.assertEqual(c.oracle('scalar', 0), '734\nTrue\n')
        self.assertEqual(c.oracle('weighted', 0), '17\n')
        self.assertNotEqual(c.oracle('weighted', 100), c.oracle('stateful', 100))


if __name__ == '__main__': unittest.main()
