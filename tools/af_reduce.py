"""AF reducer: verify canonical compressed raw, aggregate independent rounds.
python3 tools/af_reduce.py /home/na/benchmarks/minipy-s/af-capacity OUTPUT
No pooling request repetitions into independent runs; mean + min/max of 3 runs.
"""
from collections import Counter, defaultdict
import gzip,hashlib,json,math,statistics,sys,time
from pathlib import Path
from af_capacity import summarize,shape


def sustained(s):
    """Finite-window debt + tail screening, not a global capacity proof."""
    a=s['start_ns']+int(s['warmup_s']*1e9);b=s['stop_ns']
    t=[r for r in s['trace'] if a<=r['mono_ns']<=b]
    qs=[r['queued'] for r in t]
    tail=(sum(qs[-3:])/len(qs[-3:])) if qs else None
    c=s['counts'];p=s['p99_first_third_ms'];q=s['p99_last_third_ms']
    return (s['measurement_s']>=8 and not s['early_stop']
        and all(c.get(x,0)==0 for x in ['rejected','failed','timeout'])
        and s['measurement_completed']==s['measurement_offered']
        and s['completion_rate']>=s['rate']*.98
        and s['backlog_slope_per_s'] is not None and s['backlog_slope_per_s']<=max(.5,s['rate']*.01)
        and p is not None and q is not None and q<=p*1.5+2
        and tail is not None and tail<=max(2,s['rate']*.05))


def stats(values):
    values=[v for v in values if v is not None]
    return {'mean':statistics.mean(values),'min':min(values),'max':max(values)} if values else None


def verify(raw):
    with gzip.open(raw,'rt') as f:
        meta=json.loads(next(f));meta.pop('kind')
        rows=[];trace=[]
        for line in f:
            r=json.loads(line);kind=r.pop('kind')
            (rows if kind=='request' else trace).append(r)
    assert len(rows)==meta['offered']
    assert meta['scheduled']==meta['offered']+meta['cancelled_unoffered']
    assert [r['id'] for r in rows]==list(range(len(rows)))
    assert meta['driver_affinity']==[4] and meta['service_affinity']==[2]
    done=[r for r in rows if r['status']=='completed']
    for r in rows:
        assert r['status'] in ['completed','failed','rejected','timeout']
        assert r['offered_ns']>=r['planned_ns']
        assert r['planned_ns']==meta['start_ns']+round(r['id']/meta['rate']*1e9)
        assert (r['case'],r['rows'])==shape(meta['workload'],r['id'])
        if r['status']=='completed':
            assert r['planned_ns']<=r['dispatch_ns']<=r['start_ns']<=r['end_ns']<=r['response_ns']
            assert r['latency_ns']==r['response_ns']-r['planned_ns']
            assert r['queue_ns']==r['start_ns']-r['planned_ns']
            assert r['service_ns']==r['end_ns']-r['start_ns']
    if meta['final']:
        tools=sum(r['case']!='scalar' for r in done)
        assert meta['final']['loads']==meta['final']['records']==tools
        assert meta['final']['cpu_ns']==meta['service_cpu_ns']>=0
    # Independent oracle uses straightforward integer recurrence, not runtime output.
    from functools import lru_cache
    @lru_cache(None)
    def oracle(c,n):
        if c=='scalar':return '734\nTrue\n'
        total=0;prev=0
        for i in range(n):
            price,quantity=i*37%1000,i%9-2
            if quantity>0 and price<=700 and (c!='stateful' or price>=prev):
                total+=price*quantity
                if c=='stateful':prev=price
        return f'{total+17}\n'
    assert all(r['stdout']==oracle(r['case'],r['rows']) for r in done)
    # A trace records arrivals before polling responses at the same iteration.
    assert all(t['queued']<=64 and t['inflight']<=16 and t['backlog']==t['queued']+t['inflight'] for t in trace)
    for left,right in zip(trace,trace[1:]):
        assert all(right[k]>=left[k] for k in ['mono_ns','offered','completed','rejected'])
    recomputed=summarize(rows,trace,meta)
    observed=json.loads(raw.with_name(raw.name.removesuffix('.jsonl.gz')+'.json').read_text())
    assert recomputed==observed,(raw,'summary drift')
    return recomputed,{'path':str(raw),'sha256':hashlib.sha256(raw.read_bytes()).hexdigest(),'compressed_bytes':raw.stat().st_size,'requests':len(rows),'counts':dict(Counter(r['status'] for r in rows))}


def main(canonical,out):
    begin=time.monotonic();out.mkdir(parents=True,exist_ok=True)
    plan=json.loads((canonical/'plan.json').read_text())
    expected={f'confirm-{r}-{c["arm"]}-{c["workload"]}-{rate:g}' for c in plan['cells'] for rate in c['rates'] for r in range(3)}
    files=sorted(canonical.glob('confirm-*.jsonl.gz'))
    assert {f.name.removesuffix('.jsonl.gz') for f in files}==expected,'missing or unexpected confirmation'
    runs=[];index=[];totals=Counter();groups=defaultdict(list)
    for raw in files:
        s,i=verify(raw);s['sustained_v2']=sustained(s)
        s['round']=int(s['key'].split('-')[1]);groups[(s['workload'],s['arm'],s['rate'])].append(s)
        runs.append(s);index.append(i);totals.update(s['counts'])
    points=[]
    for (workload,arm,rate),ss in sorted(groups.items()):
        assert sorted(s['round'] for s in ss)==[0,1,2]
        p={'workload':workload,'engine':{'interpreter':'minipy-interpreter','native':'minipy-native','monty':'monty','fork':'cpython-fork-json','cow':'pysolate-cow'}[arm],
           'arm':arm,'offered_rate':rate,'independent_n':3,'representation':'typed-records' if arm in ['interpreter','native'] else 'monty-objects' if arm=='monty' else 'json-dict-simplenamespace',
           'pairing_group':'af-typed-monty' if arm in ['interpreter','native','monty'] else 'af-json-object',
           'sustained_rounds':sum(s['sustained_v2'] for s in ss),'early_stop_rounds':sum(s['early_stop'] for s in ss),'counts':dict(sum((Counter(s['counts']) for s in ss),Counter())),
           'round_keys':[s['key'] for s in ss],
           'status':'sustained_observed' if all(s['sustained_v2'] for s in ss) else 'overload' if any(s['early_stop'] or s['counts'].get('rejected',0) or s['completion_rate']<rate*.98 or s['backlog_slope_per_s']>max(.5,rate*.01) for s in ss) else 'tail_or_debt_unstable'}
        for metric in ['completion_rate','service_cpu_ms_per_completed','driver_cpu_ms_per_offered','backlog_slope_per_s','max_backlog','measurement_s','measurement_offered','measurement_completed']:
            p[metric]=stats([s[metric] for s in ss])
        for metric in ['latency_ms','queue_ms','service_ms','arrival_lateness_ms']:
            p[metric]={q:stats([s[metric][q] for s in ss]) for q in ['p50','p95','p99']}
        p['rejection_fraction']=stats([s['counts'].get('rejected',0)/s['offered'] for s in ss])
        points.append(p)
    capacities=[]
    for cell in plan['cells']:
        ps=[p for p in points if p['arm']==cell['arm'] and p['workload']==cell['workload']]
        good=[p['offered_rate'] for p in ps if p['status']=='sustained_observed']
        lower=max(good) if good else None
        bad=[p['offered_rate'] for p in ps if p['status']=='overload' and (lower is None or p['offered_rate']>lower)]
        capacities.append({'workload':cell['workload'],'arm':cell['arm'],'sustained_lower_req_s':lower,'overload_observed_req_s':min(bad) if bad else None,
                           'meaning':'Measured finite-window sustainable lower bound and tested overload point; not an exact maximum.'})
    result={'schema_version':1,'cohort':'af-open-loop-v1','lifecycle':'source-once-per-request; runtime initialized once per run; no prepared-cache','independent_n':3,
        'contract':{'workloads':{'cpu':{'case':'weighted','rows':10000,'tool_delay_us':0},'mixed':{'mix':{'scalar0':24,'weighted100':8,'stateful100':4,'weighted10000':2,'stateful100000':2},'period_requests':40,'permutation':'(id*17)%40','tool_delay_us':1000}},
         'arrival':'constant-rate planned CLOCK_MONOTONIC arrival, independent driver physical core','queue_limit':64,'inflight_limit':16,'service_cpu_affinity':[2],'driver_cpu_affinity':[4],
         'warmup_s':3,'arrival_duration_s':15,'measurement_s_nominal':12,'drain_limit_s':30,'overload_stop':'queue >=48 at 3 consecutive 1Hz samples, after warmup+3sec; stop future arrivals, drain outstanding',
         'latency':'response observed by driver minus PLANNED arrival; completed measurement-arrival cohort including completions during drain',
         'queue':'worker execution start minus PLANNED arrival, includes external FIFO + IPC/runtime dispatch',
         'throughput':'actual response completions inside [warmup,end-of-arrivals), divided by that actual window; drain completions excluded',
         'cpu':'all offered admitted requests + drain, per completed request; runtime initialization excluded; includes service Host/native parent/reaped children, driver separate',
         'statistics':'mean/min/max of independent-run nearest-rank percentiles; never pooled percentiles',
         'sustained_v2':'full >=8sec window; no rejects/errors/timeouts; all arrival-cohort complete; completion rate >=98% offered; backlog slope <=max(0.5,1% offered) req/s; last-third p99 <=1.5*first-third p99+2ms; mean last 3 WAITING queue samples <=max(2,5% offered). Engineering screen, not an SLA or indefinite stability proof.',
         'loss_display':'plot completed latency together with rejected/timeout/failed fractions and early-stop flags; censored/missing latency never 0; finite p99 conditional on completions'},
        'capacities':capacities,'points':points,'runs':runs,'accounting':{'planned_confirmation_runs':90,'completed_confirmation_runs':len(runs),'request_counts':dict(totals),'offered':sum(totals.values()),'cancelled_unoffered':sum(s['cancelled_unoffered'] for s in runs)},
        'storage':{'canonical_raw':str(canonical),'raw_index':'raw-index.json','raw_copies_in_git':False,'excluded':'excluded-linear-trace, 4 superseded high-rate round0 points not imported'},
        'validity':{'v1_low_rate_reuse':'11 CPU round0 points with <=675 requests retained. O(N) 1Hz driver trace overhead negligible there; high-rate MiniPy points were replaced only. All later rows use constant-time v2 driver. Source/version identities retained.',
                    'stable_v1':'preserved on each run; falsely counts natural in-flight Tool waits as queue debt. v2 uses actual waiting queue; no timing/accounting/execution change.',
                    'reference':'Existing AD/S/W Rust/ASM full and resident reference records remain valid and unchanged; not spliced into new sustained-capacity curves.'}}
    (out/'import-summary.json').write_text(json.dumps(result,indent=2)+'\n')
    (out/'raw-index.json').write_text(json.dumps({'canonical':str(canonical),'files':index,'compressed_bytes':sum(i['compressed_bytes'] for i in index)},indent=2)+'\n')
    (out/'reduction-check.json').write_text(json.dumps({'verified_runs':len(runs),'raw_requests':sum(totals.values()),'all_raw_summaries_exact_match':True,'all_outputs_oracle_match':True,'all_tool_totals_match':True,'seconds':time.monotonic()-begin},indent=2)+'\n')
    print(json.dumps({'capacities':capacities,'accounting':result['accounting'],'raw_mb':sum(i['compressed_bytes'] for i in index)/1e6,'reduction_s':time.monotonic()-begin},indent=2))

if __name__=='__main__':main(Path(sys.argv[1]),Path(sys.argv[2]))
