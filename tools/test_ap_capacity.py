"""AP selector/accounting tests. Fixtures test decisions, never performance."""
import copy
import json
from pathlib import Path
import unittest
from ap_capacity import choose, nodes, classify, POLICY

DATA=json.loads((Path(__file__).resolve().parents[1]/'bench/results/af-capacity/import-summary.json').read_text())


class APTests(unittest.TestCase):
    def setUp(self):
        self.runs=[copy.deepcopy(s) for s in DATA['runs'] if s['arm']=='interpreter' and s['workload']=='cpu']
        self.scale=3800

    def add_platform(self):
        seed=next(s for s in self.runs if s['rate']==4)
        for rate in [950,1900,2850]:
            s=copy.deepcopy(seed);s['rate']=rate
            # Keep the selector-only fixture stable without manufacturing data.
            s['completion_rate']=rate;s['backlog_slope_per_s']=0
            self.runs.append(s)

    def test_platform_before_binary_threshold(self):
        self.assertEqual(choose(self.runs,self.scale),(950,'platform'))
        self.add_platform()
        self.assertEqual(choose(self.runs,self.scale),(4400,'capacity-bracket'))

    def test_geometric_extension_when_no_overload(self):
        self.add_platform()
        rr=[s for s in self.runs if s['rate']<5000]
        self.assertEqual(choose(rr,self.scale),(5700,'expand-up'))

    def test_ambiguous_midpoint_never_replays_cached_exploration(self):
        self.add_platform()
        s=copy.deepcopy(next(s for s in self.runs if s['rate']==3800))
        s['rate']=4400
        s['p99_last_third_ms']=1000
        s['completion_rate']=4400
        self.runs.append(s)
        self.assertEqual(classify(s),'tail_or_debt_unstable')
        rate,reason=choose(self.runs,self.scale)
        self.assertEqual(reason,'ambiguous-bracket-gap')
        self.assertNotIn(rate,{s['rate'] for s in self.runs})

    def test_geometric_extension_when_no_stable(self):
        rr=[s for s in self.runs if s['rate']==5000]
        # Coverage is a prerequisite; use those same rates to test down expansion.
        for rate in [950,1900,2850]:
            s=copy.deepcopy(rr[0]);s['rate']=rate;rr.append(s)
        self.assertEqual(choose(rr,self.scale),(475,'expand-down'))

    def test_rejections_cannot_pass_stability(self):
        s=copy.deepcopy(next(s for s in self.runs if s['rate']==3800))
        s['counts']['rejected']=1
        self.assertEqual(classify(s),'overload')

    def test_disagreeing_rounds_are_not_sustained(self):
        s=copy.deepcopy(next(s for s in self.runs if s['rate']==3800))
        s['counts']['rejected']=1
        self.runs.append(s)
        n=next(n for n in nodes(self.runs) if n['rate']==3800)
        self.assertTrue(n['noisy']);self.assertFalse(n['good']);self.assertFalse(n['bad'])

    def test_tail_refinement_not_only_capacity(self):
        self.add_platform()
        for s in self.runs:
            if s['rate']==5000:s['rate']=3900
        # Bracket is now narrow, but a large p99 rise still requires probes.
        rate,reason=choose(self.runs,self.scale)
        self.assertEqual(reason,'overload-region')
        s=copy.deepcopy(next(s for s in self.runs if s['rate']==3900))
        s['rate']=4485;self.runs.append(s)
        for s in self.runs:
            if s['rate']==2850:s['latency_ms']['p99']=100
        rate,reason=choose(self.runs,self.scale)
        self.assertEqual(reason,'p99-rise')
        self.assertNotIn(rate,{s['rate'] for s in self.runs})

    def test_budget_and_tail_floor_are_explicit(self):
        self.assertEqual(POLICY['confirmation_requests'],1000)
        self.assertEqual(POLICY['confirmation_rounds'],3)
        self.assertLessEqual(POLICY['campaign_seconds'],4200)


class PublicTests(unittest.TestCase):
    def setUp(self):
        self.source=copy.deepcopy(DATA)
        self.source.update(cohort='ap-adaptive-af-frozen-v1',policy=POLICY,
                           statistics='fixture projection test',display='markers')
        lookup={s['key']:s for s in self.source['runs']}
        for p in self.source['points']:
            p['measurement_completed_per_round']=[lookup[k]['measurement_completed'] for k in p['round_keys']]

    def test_projection_reconciles_and_strips_private_trace(self):
        from ap_public import project
        result=project(self.source)
        self.assertEqual(len(result['points']),30)
        self.assertTrue(all('argv' not in s and 'trace' not in s for s in result['runs']))

    def test_reject_duplicate_or_incorrect_counts(self):
        from ap_public import project
        self.source['runs'].append(self.source['runs'][0])
        with self.assertRaisesRegex(AssertionError,'duplicate'):project(self.source)
        self.source['runs'].pop()
        self.source['accounting']['offered']+=1
        with self.assertRaises(AssertionError):project(self.source)

    def test_reject_percentile_drift(self):
        from ap_public import project
        self.source['points'][0]['latency_ms']['p99']['mean']+=1
        with self.assertRaises(AssertionError):project(self.source)

    def test_sparse_rounds_cannot_inherit_sample_supported_plot_label(self):
        from ap_public import project
        p=next(p for p in self.source['points'] if p['arm']=='cow' and p['workload']=='cpu')
        p.update(plot_round_keys=p['round_keys'],plot_independent_n=3,
                 plot_latency_ms=p['latency_ms'],p99_precision='sample-supported')
        with self.assertRaises(AssertionError):project(self.source)


if __name__=='__main__':unittest.main()
