"""Deterministic AD plan/accounting tests. Real execution is separate WSL smoke."""
import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('campaign', Path(__file__).with_name('ad_campaign.py'))
assert spec is not None and spec.loader is not None
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
                  stdout=c.oracle(item['case'], item['rows']), wall_ns=15, loads=0, records=0,
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

    def test_first_tool_count_rejected(self):
        item, rows = self.first_fixture()
        rows[1]['loads'] = 1
        with self.assertRaises(AssertionError): c.validate(rows, item)

    def test_memory_pid_pairing_and_sum(self):
        item = next(i for i in c.schedule(Path('/x')) if i['kind'] == 'hold' and i['arm'] == 'fork' and i['reps'] == 1)
        common = dict(cohort=c.COHORT, command_key=item['key'], round=item['round'])
        def row(phase, pids):
            return dict(common, kind='hold', phase=phase, physical=dict(tree_pids=pids, pss_bytes=2*len(pids), rss_bytes=3*len(pids), per_pid=[dict(pid=p, pss_bytes=2, rss_bytes=3) for p in pids]))
        rows = [dict(common, kind='initialization'), row('startup',[1,2]),row('empty_tree',[1,2,3]),row('load_wait',[1,2,3]),row('record_wait',[1,2,3]),row('reclaimed',[1,2])]
        c.validate(rows, item)
        bad=copy.deepcopy(rows); bad[3]['physical']['tree_pids']=[1,2,4]
        with self.assertRaises(AssertionError): c.validate(bad,item)
        bad=copy.deepcopy(rows); bad[3]['physical']['pss_bytes']=99
        with self.assertRaises(AssertionError): c.validate(bad,item)

    def test_mixed_cpu_not_duplicated_and_units(self):
        import runpy
        helper = runpy.run_path(str(Path(__file__).with_name('s_wsl_summary.py')))
        shapes = [('scalar',0),('weighted',100),('stateful',100),('weighted',10000),('stateful',100000)]
        weights = [24,8,4,2,2]
        plan = dict(shapes=shapes, mixed={'light_weights':weights})
        rows = []
        for ri in range(3):
            for (case,n), copies in zip(shapes,weights):
                rows += [dict(round=ri,sample=1,case=case,rows=n,mix='light',latency_ns=5000000,batch_wall_ns=50000000,batch_cpu_ns=8000000,overloaded=False)] * copies
        result = helper['by_round'](rows, lambda rs: helper['mixed'](rs, plan))
        self.assertEqual(result['requests'],120)
        self.assertEqual(result['windows'],3)
        self.assertEqual(result['cpu_us_per_task'],200)
        self.assertEqual(result['finite_requests_per_s'],400)
        self.assertEqual(result['drain_requests_per_s'],800)
        self.assertEqual(result['short_p99_us'],5000)
        self.assertEqual(result['round_ranges']['cpu_us_per_task'],[200,200])
        missing = helper['shapes']([dict(wall_ns=1000,cpu_ns=2000)])
        self.assertIsNone(missing['conversion_mean_us'])
        self.assertIsNone(missing['prepare_mean_us'])

    def test_output_independent_oracle(self):
        self.assertEqual(c.oracle('scalar', 0), '734\nTrue\n')
        self.assertEqual(c.oracle('weighted', 0), '17\n')
        self.assertNotEqual(c.oracle('weighted', 100), c.oracle('stateful', 100))


if __name__ == '__main__': unittest.main()
