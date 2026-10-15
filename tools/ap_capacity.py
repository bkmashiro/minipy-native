"""AP bounded adaptive extension, importing the unchanged frozen AF driver.
Run under compute.lock, CPU4: python3 ap_capacity.py BASE campaign|reduce
New canonical raw lives next to AF raw with ap-* names; no raw copies.
"""
from collections import Counter, defaultdict
import hashlib
import json
import math
import os
from pathlib import Path
import statistics
import sys
import time

import af_capacity as af
from af_reduce import sustained, stats, verify

POLICY = dict(campaign_seconds=4200, max_new_offered=6_000_000,
              bracket_relative=.05, platform_relative=.25, tail_ratio=1.5,
              tail_width_relative=.05, confirmation_requests=1000,
              confirmation_rounds=3, drift_cpu_fraction=.25)


def save(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


def classify(s):
    if sustained(s):
        return 'sustained_observed'
    if (s['early_stop'] or any(s['counts'].get(k, 0) for k in ['rejected', 'failed', 'timeout'])
            or s['completion_rate'] < .98*s['rate']
            or s['backlog_slope_per_s'] > max(.5, .01*s['rate'])):
        return 'overload'
    return 'tail_or_debt_unstable'


def nodes(runs):
    grouped = defaultdict(list)
    for s in runs:
        grouped[s['rate']].append(s)
    result = []
    for rate, ss in sorted(grouped.items()):
        states = [classify(s) for s in ss]
        result.append(dict(rate=rate, p99=statistics.median(s['latency_ms']['p99'] for s in ss),
                           good=all(x == 'sustained_observed' for x in states),
                           bad=all(x == 'overload' for x in states),
                           noisy=len(set(states)) > 1,
                           spread=max(s['latency_ms']['p99'] for s in ss) /
                           max(1e-9, min(s['latency_ms']['p99'] for s in ss)), runs=ss))
    return result


def choose(runs, scale):
    """Prioritize coverage, threshold interval, then curved/tail/noisy intervals."""
    ns = nodes(runs)
    rates = {n['rate'] for n in ns}
    # Three interior platform probes, independent of tail/threshold shape.
    for fraction in [.25, .5, .75]:
        r = round(scale*fraction, 4)
        if r not in rates:
            return r, 'platform'
    good = [n for n in ns if n['good']]
    bad = [n for n in ns if n['bad']]
    if not good:
        return round(ns[0]['rate']/2, 4), 'expand-down'
    lo = max(n['rate'] for n in good)
    upper = [n['rate'] for n in bad if n['rate'] > lo]
    if not upper:
        return round(ns[-1]['rate']*1.5, 4), 'expand-up'
    hi = min(upper)
    if hi/lo-1 > POLICY['bracket_relative']:
        mid=round((lo+hi)/2, 4)
        if mid not in rates:
            return mid, 'capacity-bracket'
        # A sampled midpoint may be tail-unstable, neither good nor overload.
        # Split an unmeasured subinterval rather than replaying a cached run.
        inside=[n['rate'] for n in ns if lo<=n['rate']<=hi]
        a,b=max(zip(inside,inside[1:]),key=lambda ab:ab[1]-ab[0])
        if (b-a)/scale > POLICY['tail_width_relative']:
            return round((a+b)/2,4), 'ambiguous-bracket-gap'
    # Need a measured overload region, not only one right endpoint.
    if len([n for n in ns if n['rate'] >= hi]) < 2:
        return round(hi*1.15, 4), 'overload-region'
    candidates = []
    for a, b in zip(ns, ns[1:]):
        width = (b['rate']-a['rate'])/scale
        ratio = max(a['p99'], b['p99'])/max(1e-9, min(a['p99'], b['p99']))
        if width > POLICY['tail_width_relative'] and ratio >= POLICY['tail_ratio']:
            candidates.append((ratio*width, round((a['rate']+b['rate'])/2, 4)))
    if candidates:
        return max(candidates)[1], 'p99-rise'
    return None, 'ambiguous-bracket-unresolved' if hi/lo-1 > POLICY['bracket_relative'] else 'spatial-target-met'


def seeds(base):
    summary = json.loads((base/'af-capacity/export/import-summary.json').read_text())
    return summary


def identity(base, out):
    old = json.loads((base/'af-capacity/identity.json').read_text())
    matched = {}
    for name, spec in old['files'].items():
        p = base/name
        digest = hashlib.sha256(p.read_bytes()).hexdigest()
        assert digest == spec['sha256'], ('frozen identity changed', name)
        matched[name] = digest
    # The Go adapter reads base/source/fixtures, not the archived source path.
    for p in (base/'af-capacity/source/fixtures').glob('*.py'):
        assert p.read_bytes() == (base/'source/fixtures'/p.name).read_bytes()
    index = json.loads((base/'af-capacity/export/raw-index.json').read_text())
    for item in index['files']:
        assert hashlib.sha256(Path(item['path']).read_bytes()).hexdigest() == item['sha256']
    record = dict(frozen_commit='5e047679d19e002f064de6dc52516b5d6220eea1',
                  files=matched, original_raw_hashes=len(index['files']),
                  kernel=os.uname().release, python=sys.version,
                  cpu_model=next(x for x in Path('/proc/cpuinfo').read_text().splitlines() if x.startswith('model name')),
                  topology={str(i):Path(f'/sys/devices/system/cpu/cpu{i}/topology/thread_siblings_list').read_text().strip() for i in [2,4]},
                  affinity=sorted(os.sched_getaffinity(0)), policy=POLICY,
                  controller_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest())
    save(out, record)


def campaign(base):
    assert os.sched_getaffinity(0) == {4}
    out = base/'ap-capacity'; out.mkdir(exist_ok=True)
    identity(base, out/('identity-resume.json' if (out/'identity-before.json').exists() else 'identity-before.json'))
    old = seeds(base)
    cells = [(c['arm'], c['workload'], c['sustained_lower_req_s']) for c in old['capacities']]
    # Rotate the expensive arm to avoid systematically receiving leftover budget.
    cells.sort(key=lambda c: (c[0] != 'cow', c[1], c[0]))
    original = {(a,w):[s for s in old['runs'] if s['arm']==a and s['workload']==w] for a,w,_ in cells}
    state_path = out/'state.json'
    state = json.loads(state_path.read_text()) if state_path.exists() else dict(policy=POLICY, decisions=[], curves={}, new_keys=[], elapsed_s=0)
    started = time.monotonic(); initial_elapsed = state['elapsed_s']
    new_runs = []
    for key in state['new_keys']:
        new_runs.append(json.loads((base/'af-capacity'/(key+'.json')).read_text()))

    def remaining():
        return POLICY['campaign_seconds']-initial_elapsed-(time.monotonic()-started)

    def checkpoint():
        state['elapsed_s'] = initial_elapsed+time.monotonic()-started
        save(state_path, state)

    def run(a,w,rate,phase,reason,round_id=0):
        duration = max(15, 3+POLICY['confirmation_requests']/rate) if phase=='confirm' else max(11, min(30,3+100/rate)) if phase=='explore' else 15
        # Reserve conservative launch+drain allowance before starting a process.
        projected = math.ceil(duration*rate)
        if remaining() < duration+35 or sum(s['offered'] for s in new_runs)+projected > POLICY['max_new_offered']:
            return None
        key=f'ap-{phase}-{a}-{w}-{rate:g}-{round_id}'
        s=af.run(base,a,w,rate,duration,3,key)
        if key not in state['new_keys']:
            new_runs.append(s);state['new_keys'].append(key)
            state['decisions'].append(dict(key=key,reason=reason,rate=rate,phase=phase))
        checkpoint()
        print(key, classify(s), s['measurement_completed'], s['latency_ms']['p99'], 'remaining_s',round(remaining()),flush=True)
        return s

    # Per-curve matched anchor. CPU/request is less queue-sensitive than tail.
    for a,w,scale in cells:
        name=a+'-'+w
        if name in state['curves']:
            continue
        s=run(a,w,scale,'anchor','AF stable lower drift probe')
        if s is None: break
        baseline=[r for r in original[(a,w)] if r['rate']==scale]
        cpu=statistics.median(r['service_cpu_ms_per_completed'] for r in baseline)
        drift=abs(s['service_cpu_ms_per_completed']/cpu-1) > POLICY['drift_cpu_fraction']
        tail_noise=s['latency_ms']['p99'] > 2*max(r['latency_ms']['p99'] for r in baseline)
        second=run(a,w,scale,'anchor','repeat suspected drift/noisy tail',1) if drift or tail_noise else None
        confirmed_drift=bool(drift and second and abs(second['service_cpu_ms_per_completed']/cpu-1) > POLICY['drift_cpu_fraction'])
        state['curves'][name]=dict(arm=a,workload=w,scale=scale,reuse_af=not confirmed_drift,
                                  drift_suspected=drift,drift_confirmed=confirmed_drift,tail_noise=tail_noise,
                                  anchor_keys=[s['key']]+([second['key']] if second else []))
        checkpoint()

    def curve_runs(a,w):
        curve=state['curves'][a+'-'+w]
        return (original[(a,w)] if curve['reuse_af'] else []) + [s for s in new_runs if s['arm']==a and s['workload']==w]

    # Round-robin, at most one new spatial point/curve per pass.
    for sweep in range(7):
        progress=False
        for a,w,scale in cells:
            if a+'-'+w not in state['curves']: continue
            rr=curve_runs(a,w)
            if not state['curves'][a+'-'+w]['reuse_af']:
                missing=[r for r in sorted({s['rate'] for s in original[(a,w)]}) if r not in {s['rate'] for s in rr}]
            else: missing=[]
            rate,reason=(missing[0],'replace drifted seed') if missing else choose(rr,scale)
            if rate is None: continue
            if run(a,w,rate,'explore',reason) is not None: progress=True
        # First ensure all curves get platform/bracket probes. Then confirm
        # current bracket endpoints before spending budget on tail refinement.
        if not progress: break

    # Confirm current stable/unstable endpoints in independent fresh processes.
    # A full request target is used per run, also on low-throughput adapters.
    targets={}
    for a,w,scale in cells:
        if a+'-'+w not in state['curves']: continue
        ns=nodes(curve_runs(a,w));good=[n['rate'] for n in ns if n['good']]
        lo=max(good) if good else None
        bad=[n['rate'] for n in ns if n['bad'] and (lo is None or n['rate']>lo)]
        targets[(a,w)]=([lo] if lo else [])+([min(bad)] if bad else [])
    for round_id in range(POLICY['confirmation_rounds']):
        for a,w,scale in cells:
            if a+'-'+w not in state['curves']: continue
            for rate in targets[(a,w)]:
                run(a,w,rate,'confirm','independent knee endpoint confirmation',round_id)

    # Continue spatial refinement, retesting inconsistent or >2x tail spread.
    for sweep in range(80):
        progress=False
        for a,w,scale in cells:
            if a+'-'+w not in state['curves']: continue
            rr=curve_runs(a,w)
            ns=nodes(rr)
            good=[n for n in ns if n['good']]
            best=max(good,key=lambda n:n['rate']) if good else None
            full=[s for s in best['runs'] if s['measurement_completed']>=1000 and not s['early_stop']] if best else []
            noisy=[n for n in ns if (n['noisy'] or (not n['good'] and not n['bad']) or n['spread']>2) and len(n['runs'])<5]
            if best and len(full)<3:
                s=run(a,w,best['rate'],'confirm','support current stable endpoint after refinement',10+len(best['runs']))
            elif noisy:
                n=noisy[0]
                s=run(a,w,n['rate'],'confirm','noise retest',len(n['runs']))
            else:
                rate,reason=choose(rr,scale)
                s=run(a,w,rate,'explore',reason) if rate is not None else None
            progress |= s is not None
        if not progress: break
    for a,w,scale in cells:
        if a+'-'+w not in state['curves']: continue
        rate,reason=choose(curve_runs(a,w),scale)
        state['curves'][a+'-'+w].update(spatial_converged=rate is None and reason=='spatial-target-met', next_rate=rate, next_reason=reason)
    checkpoint()
    identity(base,out/'identity-after.json')
    reduce(base)


def reduce(base):
    out=base/'ap-capacity';state=json.loads((out/'state.json').read_text());old=seeds(base)
    all_runs=[];index=[]
    for key in state['new_keys']:
        s,i=verify(base/'af-capacity'/(key+'.jsonl.gz'))
        s['phase']=next(d['phase'] for d in state['decisions'] if d['key']==key)
        # Independent of planned rate: offered timing span and counts in real window.
        import gzip
        with gzip.open(base/'af-capacity'/(key+'.jsonl.gz'),'rt') as f:
            records=[json.loads(line) for line in f]
        requests=[r for r in records if r['kind']=='request']
        span=(max(r['offered_ns'] for r in requests)-min(r['offered_ns'] for r in requests))/1e9
        s['actual_offered_span_rate']=(len(requests)-1)/span if span else None
        dispatched=[r for r in requests if 'dispatch_ns' in r]
        dispatch_span=(max(r['dispatch_ns'] for r in dispatched)-min(r['dispatch_ns'] for r in dispatched))/1e9 if dispatched else 0
        s['actual_dispatched_span_rate']=(len(dispatched)-1)/dispatch_span if dispatch_span else None
        s['sustained_v2']=sustained(s);s['status']=classify(s)
        all_runs.append(s);index.append(i)
    effective=[]
    for c in state['curves'].values():
        rr=[s for s in all_runs if s['arm']==c['arm'] and s['workload']==c['workload']]
        if c['reuse_af']:
            rr += [{**s,'phase':'af-reused'} for s in old['runs'] if s['arm']==c['arm'] and s['workload']==c['workload']]
        effective.extend(rr)
    groups=defaultdict(list)
    for s in effective:groups[(s['arm'],s['workload'],s['rate'])].append(s)
    points=[]
    for (a,w,rate),ss in sorted(groups.items()):
        full=[s for s in ss if s['measurement_completed']>=POLICY['confirmation_requests'] and not s['early_stop']]
        states=[classify(s) for s in ss]
        p=dict(arm=a,workload=w,offered_rate=rate,independent_n=len(ss),
               engine={'interpreter':'minipy-interpreter','native':'minipy-native','monty':'monty','fork':'cpython-fork-json','cow':'pysolate-cow'}[a],
               representation='typed-records' if a in ['native','interpreter'] else 'monty-objects' if a=='monty' else 'json-dict-simplenamespace',
               phases=sorted({s['phase'] for s in ss}), round_keys=[s['key'] for s in ss],
               counts=dict(sum((Counter(s['counts']) for s in ss),Counter())),
               status=states[0] if len(set(states))==1 else 'inconsistent',
               sustained_rounds=states.count('sustained_observed'), early_stop_rounds=sum(s['early_stop'] for s in ss),
               full_sample_rounds=len(full), p99_precision='sample-supported' if len(full)>=3 else 'exploratory-or-censored',
               measurement_completed_per_round=[s['measurement_completed'] for s in ss],
               cancelled_unoffered=sum(s['cancelled_unoffered'] for s in ss))
        p['measurement_s_per_round']=[s['measurement_s'] for s in ss]
        # Do not let a sparse legacy/exploratory run inherit the confirmation
        # floor merely because three larger runs exist at the same rate.
        plot_runs=full if len(full)>=3 else ss
        p['plot_round_keys']=[s['key'] for s in plot_runs]
        p['plot_independent_n']=len(plot_runs)
        p['plot_latency_ms']={q:stats([s['latency_ms'][q] for s in plot_runs]) for q in ['p50','p95','p99']}
        for m in ['latency_ms','queue_ms','service_ms','arrival_lateness_ms']:
            p[m]={q:stats([s[m][q] for s in ss]) for q in ['p50','p95','p99']}
        for m in ['completion_rate','service_cpu_ms_per_completed','driver_cpu_ms_per_offered','backlog_slope_per_s','max_backlog','measurement_s','actual_offered_span_rate','actual_dispatched_span_rate']:
            p[m]=stats([s.get(m) for s in ss])
        p['rejection_fraction']=stats([s['counts'].get('rejected',0)/s['offered'] for s in ss])
        points.append(p)
    capacities=[]
    for c in state['curves'].values():
        pp=[p for p in points if p['arm']==c['arm'] and p['workload']==c['workload']]
        good=[p['offered_rate'] for p in pp if p['status']=='sustained_observed']
        lo=max(good) if good else None
        bad=[p['offered_rate'] for p in pp if p['status']=='overload' and (lo is None or p['offered_rate']>lo)]
        hi=min(bad) if bad else None
        confirmed_good=[p['offered_rate'] for p in pp if p['status']=='sustained_observed' and p['full_sample_rounds']>=3]
        endpoint_points=[p for p in pp if p['offered_rate'] in [lo,hi]]
        capacities.append(dict(**c,sustained_lower_req_s=lo,overload_observed_req_s=hi,
                               confirmed_sustained_lower_req_s=max(confirmed_good) if confirmed_good else None,
                               bracket_repeated=bool(lo and hi and all(p['independent_n']>=3 for p in endpoint_points)),
                               relative_bracket=hi/lo-1 if hi and lo else None,
                               meaning='Finite-window observed bracket; spatial convergence is separate from repeated sample support.'))
    totals=sum((Counter(s['counts']) for s in all_runs),Counter())
    effective_totals=sum((Counter(s['counts']) for s in effective),Counter())
    contract={**old['contract'], 'arrival_duration_s':'Variable by phase and rate; exact per-run duration_s retained.',
              'measurement_s_nominal':'Variable. AF reused/anchor 12s, exploration >=8s, confirmation max(12,1000/rate)s before bounded early stop.',
              'statistics':'Per-run nearest-rank then mean/min/max, variable independent_n. Every run duration and phase retained. No p99 confidence claim.'}
    summary=dict(schema_version=2,cohort='ap-adaptive-af-frozen-v1', policy=POLICY,
                 lifecycle=old['lifecycle'],contract=contract,capacities=capacities,points=points,runs=effective,
                 accounting=dict(new_runs=len(all_runs),new_points=len({(s['arm'],s['workload'],s['rate']) for s in all_runs}),
                                 request_counts=dict(effective_totals),offered=sum(effective_totals.values()),
                                 cancelled_unoffered=sum(s['cancelled_unoffered'] for s in effective),
                                 new_request_counts=dict(totals),new_offered=sum(totals.values()),
                                 new_cancelled_unoffered=sum(s['cancelled_unoffered'] for s in all_runs),elapsed_s=state['elapsed_s']),
                 statistics='Mean/min/max of independent run nearest-rank percentiles, not pooled. Exploratory/censored flags mandatory. 1000 completions is a screening floor, not a p99 confidence guarantee.',
                 display='Measured markers and straight segments only. Keep rejects/timeouts/failures/early stops/cancelled-unoffered visible. Old short slow runs are exploratory, not precise tails.',
                 decisions=state['decisions'])
    save(out/'import-summary.json',summary)
    save(out/'raw-index.json',dict(canonical=str(base/'af-capacity'),files=index,compressed_bytes=sum(i['compressed_bytes'] for i in index)))
    save(out/'reduction-check.json',dict(verified_new_runs=len(all_runs),all_raw_summaries_exact_match=True,all_outputs_oracle_match=True,all_tool_totals_match=True,counts=dict(totals)))
    print(json.dumps(dict(accounting=summary['accounting'],points=len(points),capacities=capacities),indent=2),flush=True)


if __name__=='__main__':
    (campaign if sys.argv[2]=='campaign' else reduce)(Path(sys.argv[1]).resolve())
