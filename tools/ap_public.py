"""Produce a small public AP projection, verifying run/point accounting.
No raw requests, traces, argv, host paths or source identity paths are published.
python3 tools/ap_public.py INPUT OUTPUT
"""
from collections import Counter
import json
from pathlib import Path
import sys

RUN_KEYS = ['key','arm','workload','rate','phase','duration_s','warmup_s','offered',
            'scheduled','cancelled_unoffered','counts','measurement_s',
            'measurement_offered','measurement_completed','completion_rate',
            'latency_ms','queue_ms','service_ms','arrival_lateness_ms',
            'actual_offered_span_rate','actual_dispatched_span_rate',
            'service_cpu_ms_per_completed','driver_cpu_ms_per_offered',
            'early_stop','stable_observed','sustained_v2','status',
            'backlog_slope_per_s','max_backlog','p99_first_third_ms','p99_last_third_ms']


def project(source):
    assert source['cohort']=='ap-adaptive-af-frozen-v1'
    runs=source['runs'];lookup={s['key']:s for s in runs}
    assert len(lookup)==len(runs), 'duplicate independent run'
    counts=sum((Counter(s['counts']) for s in runs),Counter())
    assert dict(counts)==source['accounting']['request_counts']
    assert sum(counts.values())==sum(s['offered'] for s in runs)==source['accounting']['offered']
    assert sum(s['cancelled_unoffered'] for s in runs)==source['accounting']['cancelled_unoffered']
    used=[]
    for p in source['points']:
        ss=[lookup[k] for k in p['round_keys']];used+=p['round_keys']
        assert len(ss)==p['independent_n']
        assert all((s['arm'],s['workload'],s['rate'])==(p['arm'],p['workload'],p['offered_rate']) for s in ss)
        assert dict(sum((Counter(s['counts']) for s in ss),Counter()))==p['counts']
        assert p['measurement_completed_per_round']==[s['measurement_completed'] for s in ss]
        assert sum(s['early_stop'] for s in ss)==p['early_stop_rounds']
        if 'plot_round_keys' in p:
            plot_runs=[lookup[k] for k in p['plot_round_keys']]
            assert set(p['plot_round_keys']) <= set(p['round_keys'])
            assert len(plot_runs)==p['plot_independent_n']
            if p['p99_precision']=='sample-supported':
                assert len(plot_runs)>=3
                assert all(s['measurement_completed']>=1000 and not s['early_stop'] for s in plot_runs)
            for q in ['p50','p95','p99']:
                vv=[s['latency_ms'][q] for s in plot_runs if s['latency_ms'][q] is not None]
                st=p['plot_latency_ms'][q]
                if vv:
                    assert abs(sum(vv)/len(vv)-st['mean']) < max(1e-8,abs(st['mean'])*1e-12)
                    assert min(vv)==st['min'] and max(vv)==st['max']
        for q in ['p50','p95','p99']:
            vv=[s['latency_ms'][q] for s in ss if s['latency_ms'][q] is not None]
            st=p['latency_ms'][q]
            assert (st is None)==(not vv)
            if vv:
                assert abs(sum(vv)/len(vv)-st['mean']) < max(1e-8,abs(st['mean'])*1e-12)
                assert min(vv)==st['min'] and max(vv)==st['max']
    assert Counter(used)==Counter(lookup.keys()), 'every effective run belongs to exactly one point'
    result={k:source[k] for k in ['schema_version','cohort','policy','lifecycle','contract',
                                 'capacities','points','accounting','statistics','display']}
    result['runs']=[{k:s[k] for k in RUN_KEYS if k in s} for s in runs]
    result['excluded_attempts']=source.get('excluded_attempts',[])
    # A finite-window high stable point above an observed lower overload is
    # evidence of order/window/noise sensitivity, not a monotone capacity proof.
    result['capacities']=[dict(c) for c in result['capacities']]
    for c in result['capacities']:
        pp=[p for p in result['points'] if p['arm']==c['arm'] and p['workload']==c['workload']]
        good=[p['offered_rate'] for p in pp if p['status']=='sustained_observed']
        c['nonmonotone_overload_rates']=[p['offered_rate'] for p in pp if p['status']=='overload' and good and p['offered_rate']<max(good)]
        c['inconsistent_rates']=[p['offered_rate'] for p in pp if p['status']=='inconsistent']
        c['spatial_converged']=bool(c.get('spatial_converged',False) and not c['nonmonotone_overload_rates'] and not c['inconsistent_rates'])
        c['interpretation']='Observed finite-window points; unresolved if classifications disagree or are nonmonotone. Do not use as an exact routing threshold.'
    # This assertion protects public host-path/argv/trace exclusion.
    text=json.dumps(result)
    assert '/home/' not in text and '/Users/' not in text
    assert all('argv' not in r and 'trace' not in r for r in result['runs'])
    return result


if __name__=='__main__':
    source=json.loads(Path(sys.argv[1]).read_text())
    interrupted=Path(sys.argv[1]).parent/'controller-interruption.json'
    if interrupted.exists():
        record=json.loads(interrupted.read_text())
        source['excluded_attempts']=[dict(attempt_id=k+'-interrupted-v1',retried_run_key=k,
            status='controller-interrupted-not-persisted',request_counts=None,included_in_accounting=False,
            reason='Selector repair; partial attempt has no persisted request ledger. Completed retry is separately included.')
            for k in record['incomplete_stderr_keys']]
    result=project(source)
    Path(sys.argv[2]).write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(dict(points=len(result['points']),runs=len(result['runs']),accounting=result['accounting']),indent=2))
