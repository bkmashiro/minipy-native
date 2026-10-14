"""Focused AF arithmetic/accounting tests; no synthetic performance claims."""
import unittest
from af_capacity import shape, quantile, summarize

class AFTests(unittest.TestCase):
    def test_mix(self):
        from collections import Counter
        self.assertEqual(Counter(shape('mixed',i) for i in range(40)),{('scalar',0):24,('weighted',100):8,('stateful',100):4,('weighted',10000):2,('stateful',100000):2})
    def test_nearest_rank(self):
        self.assertEqual(quantile([4,1,3,2],.5),2)
        self.assertEqual(quantile([4,1,3,2],.99),4)
        self.assertIsNone(quantile([],.99))
    def test_accounting_and_planned_latency(self):
        rows=[dict(id=0,status='completed',planned_ns=0,offered_ns=100, response_ns=1000000,latency_ns=1000000,queue_ns=900000,service_ns=100000),
              dict(id=1,status='rejected',planned_ns=100000000,offered_ns=100000200),
              dict(id=2,status='timeout',planned_ns=200000000,offered_ns=200000200)]
        meta=dict(start_ns=0,stop_ns=1000000000,warmup_s=0,rate=3,early_stop=False,offered=3,service_cpu_ns=None,driver_cpu_ns=3000)
        s=summarize(rows,[],meta)
        self.assertEqual(s['admitted'],2)
        self.assertEqual(s['counts'],dict(completed=1,rejected=1,timeout=1))
        self.assertEqual(s['latency_ms']['p99'],1)
        self.assertEqual(s['queue_ms']['p99'],.9)
        self.assertIsNone(s['service_cpu_ms_per_completed'])
        self.assertFalse(s['stable_observed'])
    def test_completed_after_window_is_not_throughput(self):
        rows=[dict(status='completed',planned_ns=0,offered_ns=0,response_ns=2000000000,latency_ns=2000000000,queue_ns=1000000000,service_ns=1000000000)]
        meta=dict(start_ns=0,stop_ns=1000000000,warmup_s=0,rate=1,early_stop=False,offered=1,service_cpu_ns=1000,driver_cpu_ns=1000)
        s=summarize(rows,[],meta)
        self.assertEqual(s['completion_rate'],0)
        self.assertEqual(s['drained_rate'],1)
        self.assertFalse(s['stable_observed'])

class SustainedTests(unittest.TestCase):
    def test_await_is_not_waiting_debt(self):
        from af_reduce import sustained
        s=dict(start_ns=0,stop_ns=12000000000,warmup_s=0,measurement_s=12,
               trace=[dict(mono_ns=i*1000000000,queued=0,inflight=10,backlog=10) for i in range(12)],
               counts=dict(completed=120),p99_first_third_ms=600,p99_last_third_ms=590,
               early_stop=False,measurement_completed=120,measurement_offered=120,
               completion_rate=10,rate=10,backlog_slope_per_s=0)
        self.assertTrue(sustained(s))
        for change in [dict(counts=dict(completed=119,rejected=1)),
                       dict(measurement_completed=119),dict(completion_rate=9),
                       dict(backlog_slope_per_s=1),dict(p99_last_third_ms=1000),
                       dict(early_stop=True),dict(measurement_s=7),
                       dict(trace=[dict(mono_ns=i*1000000000,queued=8) for i in range(12)])]:
            with self.subTest(change=change):self.assertFalse(sustained({**s,**change}))

if __name__=='__main__':unittest.main()
