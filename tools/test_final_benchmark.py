"""Named invariants for L reduction and common-body adapter. No timing claims."""
import asyncio
import copy
import json
from pathlib import Path
import unittest
from final_python import execute, oracle, prepare
from final_summary import mixed_stat, quantile, shape_stat, reduce_memory, reduce_speed

ROOT=Path(__file__).resolve().parents[1]


class FinalBenchmarkTests(unittest.TestCase):
    def test_common_body_exact_wrapper_and_success_corpus(self):
        identity=json.loads((ROOT/'bench/results/l-final/common-cpython-identity.json').read_text())
        for case in ['scalar','weighted','stateful']:
            body=(ROOT/f'fixtures/common_{case}.py').read_text()
            self.assertEqual(identity['body'][case],body)
            wrapper=identity['wrappers'][case]
            self.assertEqual(''.join(l[4:] for l in wrapper.splitlines(keepends=True)[1:]),body)
            code=prepare(case,body)
            for n in ([0] if case=='scalar' else [0,100,10000]):
                for reuse in [None,code]:
                    result=asyncio.run(execute({'case':case,'rows':n,'body':body},reuse))
                    self.assertEqual(result['stdout'],oracle(case,n))
                    self.assertEqual((result['loads'],result['records']),(0,0) if case=='scalar' else (1,1))

    def test_nearest_rank_is_not_interpolated_or_a_confidence_interval(self):
        self.assertEqual(quantile([30,10,20],.5),20)
        self.assertEqual(quantile([30,10,20],.99),30)

    def test_batch_cpu_counted_once_and_finite_throughput(self):
        rows=[]
        shape_weights=[('scalar',0,24),('weighted',100,8),('stateful',100,4),('weighted',10000,2),('stateful',100000,2)]
        for sample,cpu in [(1,100000),(2,200000)]:
            for case,n,count in shape_weights:
                for _ in range(count):
                    rows.append(dict(round=0,sample=sample,mix='light',case=case,rows=n,
                                     latency_ns=1000,batch_cpu_ns=cpu,batch_wall_ns=20000000,
                                     overloaded=False,completed=True))
        result=mixed_stat(rows)
        self.assertEqual(result['cpu_us_per_task'],3.75)
        self.assertEqual(result['finite_window_requests_per_s'],400)
        self.assertEqual(result['drain_requests_per_s'],2000)
        self.assertEqual(result['short_p99_us'],1)
        bad=copy.deepcopy(rows);bad[0]['batch_cpu_ns']=1
        with self.assertRaises(AssertionError):mixed_stat(bad)
        with self.assertRaises(AssertionError):mixed_stat(rows[:-1])

    def test_unmeasured_phase_and_allocator_are_not_zero(self):
        row=dict(wall_ns=1000,cpu_ns=1000,completed=True)
        result=shape_stat([row])
        self.assertIsNone(result['prepare_mean_us'])
        rows=[dict(engine='cpython-fork',code='unique',count=8,phase='record_wait',rows=10000,
                   round=ri,physical={'rss_bytes':1000,'pss_bytes':500},requested_traced_bytes=None) for ri in range(3)]
        self.assertIsNone(reduce_memory(rows,False)[0]['requested_bytes'])
        rows[0]['physical']['pss_bytes']=None
        with self.assertRaises(AssertionError):reduce_memory(rows,False)

    def test_failed_request_cannot_be_promoted_to_completed_coverage(self):
        with self.assertRaises(AssertionError):reduce_speed([dict(completed=False)],True)


if __name__=='__main__':unittest.main()
