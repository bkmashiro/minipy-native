"""Bounded extension accounting: no duplicate original cells or fabricated phases."""
import unittest
from s_wsl_extension import schedule, validate, ORIGINAL
from s_wsl_first import first


class ExtensionTests(unittest.TestCase):
    def test_schedule_and_original_successes_not_repeated(self):
        curve=schedule('curve')
        self.assertEqual(len(curve),540)
        self.assertEqual(len(schedule('reference')),168)
        self.assertEqual(len(schedule('first')),72)
        self.assertTrue(all((i['tags']['case'],i['tags']['rows']) not in ORIGINAL for i in curve))
        unsupported=[i for i in curve if 'unsupported' in i]
        self.assertEqual(len(unsupported),18)
        self.assertTrue(all(i['unsupported']['response_bytes']>1048576 for i in unsupported))
        for mode in ['curve','reference','first']:
            items=schedule(mode)
            self.assertEqual(len({i['key'] for i in items}),len(items))
            self.assertEqual({i['tags']['round'] for i in items},{0,1,2})

    def test_same_run_phase_sum_missing_and_negative_rejected(self):
        item=dict(kind='first-request',expected=1,key='x',tags=dict(case='scalar',rows=0))
        r=dict(kind='first-request',completed=True,case='scalar',rows=0,wall_ns=15,
               process_startup_ns=1,runtime_init_ns=2,source_prepare_ns=3,execute_ns=4,reap_unassigned_ns=5)
        self.assertEqual(validate([r],item),[r])
        with self.assertRaises(AssertionError):validate([],item)
        with self.assertRaises(AssertionError):validate([dict(r,wall_ns=14)],item)
        with self.assertRaises(AssertionError):validate([dict(r,process_startup_ns=-1,reap_unassigned_ns=7)],item)

    def test_duplicate_sample_or_bad_output_rejected(self):
        item=dict(kind='speed',expected=2,key='x',tags=dict(case='weighted',rows=100))
        r=dict(kind='speed',completed=True,case='weighted',rows=100,sample=0)
        with self.assertRaises(AssertionError):validate([r,r],item)
        with self.assertRaises(AssertionError):validate([r,dict(r,sample=1,completed=False)],item)

    def test_real_first_request_source_output_and_phase_order(self):
        for c,n in [('scalar',0),('weighted',100),('stateful',100)]:
            r=first(c,n)
            self.assertTrue(r['completed'])
            self.assertLessEqual(int(r['init_begin_ns']),int(r['ready_ns']))
            self.assertLessEqual(int(r['ready_ns']),int(r['prepared_ns']))
            self.assertLessEqual(int(r['prepared_ns']),int(r['executed_ns']))


if __name__=='__main__':unittest.main()
