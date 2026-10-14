"""S named invariants: common adapter parity, units, finite-window CPU accounting.
Run: python3 -m unittest discover -s tools -p test_s_wsl.py
"""
import asyncio
from collections import Counter
import importlib.util
import json
from pathlib import Path
import unittest
from s_wsl_python import BODIES, base, memory, speed
from s_wsl_summary import by_round, mixed, percentile, shapes

ROOT=Path(__file__).resolve().parents[1]
PLAN=json.loads((ROOT/'bench/results/s-wsl/plan.json').read_text())


class SWslTests(unittest.TestCase):
    def test_body_and_tool_counts_for_resident_prepared_and_isolated(self):
        for case,n in [('scalar',0),('weighted',0),('weighted',100),('stateful',100)]:
            for reuse in [None,base.prepare(case,BODIES[case])]:
                r=asyncio.run(base.execute(dict(case=case,rows=n,body=BODIES[case]),reuse))
                self.assertEqual(r['stdout'],base.oracle(case,n))
                self.assertEqual((r['loads'],r['records']),(0,0) if case=='scalar' else (1,1))
        for mode in ['cpython-resident','cpython-prepared','cpython-spawn','cpython-fork']:
            r=asyncio.run(speed(dict(label=mode,engine=mode,kind='speed',case='weighted',rows=100,reps=1)))[0]
            self.assertTrue(r['completed'])
            self.assertEqual(r['corpus'],'common')

    def test_memory_phase_lifetime_and_null_physical_scope(self):
        for code in ['unique','shared']:
            rs=asyncio.run(memory(code,1))
            self.assertEqual([r['phase'] for r in rs],PLAN['memory']['phases'])
            self.assertTrue(all(r['physical']['pss_bytes']>0 for r in rs))

    def test_cpu_once_per_window_and_finite_throughput(self):
        rs=[]
        for sample,cpu in [(1,100000),(2,200000)]:
            for (case,n),count in zip(PLAN['shapes'],PLAN['mixed']['light_weights']):
                rs += [dict(round=0,sample=sample,mix='light',case=case,rows=n,latency_ns=1000,
                            batch_cpu_ns=cpu,batch_wall_ns=20000000,overloaded=False)]*count
        result=mixed(rs,PLAN)
        self.assertEqual(result['cpu_us_per_task'],3.75)
        self.assertEqual(result['finite_requests_per_s'],400)
        self.assertEqual(result['drain_requests_per_s'],2000)
        with self.assertRaises(AssertionError): mixed(rs[:-1],PLAN)
        bad=[dict(r) for r in rs];bad[0]['batch_cpu_ns']=1
        with self.assertRaises(AssertionError):mixed(bad,PLAN)

    def test_nearest_rank_and_missing_phase(self):
        self.assertEqual(percentile([30,10,20],.99),30)
        r=shapes([dict(wall_ns=1000,cpu_ns=2000)])
        self.assertEqual(r['wall_mean_us'],1)
        self.assertEqual(r['cpu_mean_us'],2)
        self.assertIsNone(r['conversion_mean_us'])
        with self.assertRaises(AssertionError):by_round([dict(round=0,wall_ns=1000,cpu_ns=1000)],shapes)

    def test_plan_expansion_matches_finite_raw_accounting(self):
        import sys
        old=sys.argv
        sys.argv=['s_wsl_campaign',str(ROOT/'bench/results/s-wsl/plan.json'),str(ROOT/'.artifacts/s-wsl/test-plan')]
        try:
            spec=importlib.util.spec_from_file_location('campaign',ROOT/'tools/s_wsl_campaign.py')
            assert spec is not None and spec.loader is not None
            module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
            work=module.schedule()
            self.assertEqual(len(work),564)
            self.assertEqual(len({r['key'] for r in work}),564)
            self.assertEqual(sum(r['expected'] for r in work),53868)
            self.assertEqual(Counter(r['kind'] for r in work),Counter(speed=240,mixed=192,hold=126,**{'secondary-hold':6}))
            unsupported=[r for r in work if module.old_abi_case(r)]
            self.assertEqual(len(unsupported),30)
            self.assertEqual(sum(r['expected'] for r in unsupported),4866)
            self.assertTrue(all(r['key'].startswith('old-') for r in unsupported))
        finally:sys.argv=old


if __name__=='__main__':unittest.main()
