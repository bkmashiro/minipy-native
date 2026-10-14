"""S deterministic accounting and reduction. Run on WSL: SCRIPT RAW_DIR PLAN.
No missing-to-zero, no CPU duplicated across batch requests, no false CIs.
"""
from collections import Counter, defaultdict
import gzip
import hashlib
import json
import math
from pathlib import Path
import statistics as st
import sys


def load(path):
    if str(path).endswith('.gz'):
        with gzip.open(path,'rt') as f: return [json.loads(l) for l in f]
    return [json.loads(l) for l in Path(path).read_text().splitlines()]


def percentile(xs,p):
    return sorted(xs)[math.ceil(len(xs)*p)-1]


def shapes(rs):
    out: dict[str, object]=dict(requests=len(rs),wall_mean_us=st.mean(r['wall_ns'] for r in rs)/1000,
             wall_median_us=st.median(r['wall_ns'] for r in rs)/1000,
             cpu_mean_us=st.mean(r['cpu_ns'] for r in rs)/1000)
    for metric in ['prepare_ns','execute_ns','conversion_ns','producer_setup_ns']:
        xs=[r[metric] for r in rs if metric in r]
        out[metric.replace('_ns','_mean_us')]=st.mean(xs)/1000 if xs else None
    return out


def mixed(rs,plan):
    batches=defaultdict(list)
    for r in rs: batches[r['round'],r['sample']].append(r)
    for rows in batches.values():
        assert len(rows)==40
        assert len({(r['batch_wall_ns'],r['batch_cpu_ns'],r['overloaded']) for r in rows})==1
        want=Counter({tuple(s):w for s,w in zip(plan['shapes'],plan['mixed'][rows[0]['mix']+'_weights'])})
        assert Counter((r['case'],r['rows']) for r in rows)==want
    first=[rows[0] for rows in batches.values()]
    short=[r['latency_ns'] for r in rs if r['rows']<=100]
    return dict(requests=len(rs),windows=len(first),cpu_us_per_task=sum(r['batch_cpu_ns'] for r in first)/len(rs)/1000,
                latency_mean_us=st.mean(r['latency_ns'] for r in rs)/1000,
                short_p50_us=percentile(short,.5)/1000,short_p95_us=percentile(short,.95)/1000,short_p99_us=percentile(short,.99)/1000,
                finite_requests_per_s=len(rs)/(sum(max(100000000,r['batch_wall_ns']) for r in first)/1e9),
                drain_requests_per_s=len(rs)/(sum(r['batch_wall_ns'] for r in first)/1e9),
                overloaded_windows=sum(r['overloaded'] for r in first))


def by_round(rs,fn):
    assert sorted({r['round'] for r in rs})==[0,1,2]
    whole=fn(rs)
    rounds=[fn([r for r in rs if r['round']==ri]) for ri in range(3)]
    whole['rounds']=rounds
    whole['round_ranges']={k:[min(r[k] for r in rounds),max(r[k] for r in rounds)] for k,v in whole.items() if isinstance(v,(float,int)) and k not in ['requests','windows']}
    return whole


def reduce(out,plan):
    root=Path(__file__).resolve().parents[1]
    frozen=json.loads((root/'bench/results/s-wsl/frozen-source.json').read_text())
    assert frozen['runtime_commit']==plan['runtime_commit']
    assert all(hashlib.sha256((root/p).read_bytes()).hexdigest()==digest for p,digest in frozen['files'].items())
    status=json.loads((out/'status.json').read_text())
    commands=json.loads((out/'commands.json').read_text())
    assert len(status)==len(commands) and len({s['key'] for s in status})==len(status)
    assert {s['key'] for s in status}=={i['key'] for i in commands}
    unsupported=[s for s in status if s['state']=='unsupported']
    assert len(unsupported)==30
    observed_failures=sum(s['observed_failure'] for s in unsupported)
    assert all(s['state'] in ['completed','unsupported'] for s in status), [s for s in status if s['state'] not in ['completed','unsupported']]
    dispositions={s['key']:s for s in status}
    rows=[]
    raw_identity={}
    for item in commands:
        if dispositions[item['key']]['state']=='unsupported':
            assert item['key'].startswith('old-') and (item['kind']=='mixed' or (item['kind']=='speed' and item['argv'][7]=='100000'))
            assert dispositions[item['key']]['scheduled_rows']==item['expected']
            continue
        path=out/(item['key']+'.jsonl.gz')
        raw_identity[path.name]=dict(sha256=hashlib.sha256(path.read_bytes()).hexdigest(),bytes=path.stat().st_size)
        rs=load(path)
        selected=[r for r in rs if r['kind']==item['kind']]
        assert len(selected)==item['expected']
        for r in rs:
            assert r['round']==item['tags']['round'] and r['command_key']==item['key']
            r.setdefault('corpus','fixture-lower-bound' if r['engine']=='reference' else 'common')
            rows.append(r)
    assert sum(i['expected'] for i in commands)==sum(s.get('scheduled_rows',s['rows']) for s in status)
    completed_rows=sum(s['rows'] for s in status)
    pyro=load(out/'common-pyodide.jsonl.gz')
    assert len(pyro)==5430 and all(r['completed'] for r in pyro)
    for r in pyro:
        r.update(corpus='common',discard=1)
    rows.extend(pyro)
    pyro_memory=load(out/'pyodide-memory.jsonl')
    assert len(pyro_memory)==24
    for r in pyro_memory: r.update(corpus='tuple-secondary',representation='tuple-secondary')
    rows.extend(pyro_memory)
    sg,mg,hg=defaultdict(list),defaultdict(list),defaultdict(list)
    init=[]
    raw_counts=Counter()
    for r in rows:
        raw_counts[r['engine'],r['kind']]+=1
        if r['kind']=='initialization': init.append(r);continue
        if r['kind'] in ['hold','secondary-hold']:
            hg[r['engine'],r['corpus'],r['code'],r['count'],r['phase']].append(r);continue
        assert r['completed']
        if r['sample']<r['discard']: continue
        mode=r.get('mode','warm' if r['engine'].endswith('prepared') else 'cold')
        if r['kind']=='speed':
            sg[r['engine'],r['corpus'],mode,r['case'],r['rows']].append(r)
        else:
            mg[r['engine'],r['corpus'],mode,r['mix'],r['delay_us'],r['inflight']].append(r)
    assert {key[0] for key in sg}==set(plan['required_arms'])
    speed=[]
    for (engine,corpus,mode,case,n),rs in sorted(sg.items()):
        per_round=40 if engine.startswith('pypy') else 20 if engine.startswith('pyodide') else 10 if engine.startswith('pysolate') else 30
        assert len(rs)==per_round*3
        assert len({(r['round'],r['sample']) for r in rs})==len(rs)
        code={r['prepared_bytes'] for r in rs if r.get('prepared_bytes') is not None}
        speed.append(dict(engine=engine,corpus=corpus,mode=mode,case=case,rows=n,prepared_bytes=code.pop() if len(code)==1 else None,**by_round(rs,shapes)))
    mixed_out=[]
    for (engine,corpus,mode,mix,delay,cap),rs in sorted(mg.items()):
        assert len(rs)==480
        mixed_out.append(dict(engine=engine,corpus=corpus,mode=mode,mix=mix,delay_us=delay,inflight=cap,**by_round(rs,lambda r:mixed(r,plan))))
    memory=[]
    for (engine,corpus,code,tasks,phase),rs in sorted(hg.items()):
        assert len(rs)==3 and sorted(r['round'] for r in rs)==[0,1,2]
        record: dict[str, object]=dict(engine=engine,corpus=corpus,code=code,tasks=tasks,phase=phase,rows=10000,independent_processes=3)
        for name in ['rss_bytes','pss_bytes']:
            xs=[r['physical'][name] for r in rs]
            assert all(x is not None for x in xs)
            record[name]=dict(mean=st.mean(xs),range=[min(xs),max(xs)],rounds=xs)
        requested=[r['allocation']['live_requested_bytes'] if 'allocation' in r else r.get('requested_traced_bytes') for r in rs]
        record['requested_bytes']=dict(mean=st.mean(requested),range=[min(requested),max(requested)]) if all(v is not None for v in requested) else None
        memory.append(record)
    expected_shape_cells=4*5*2+6*5+2*5+2*5
    expected_mixed_cells=4*2*2*2+6*2*2+2*2*2+2*2*2
    assert expected_shape_cells==90 and len(speed)==88
    assert expected_mixed_cells==72 and len(mixed_out)==64
    assert len(memory)==6*2*3*4+2*4+2*3*4+2*4==184
    assert len(init)==42 # old 24 supported request commands + 18 memory commands
    secondary=json.loads((out/'secondary-identity.json').read_text())
    assert len(secondary['commands'])==126
    owners=load(out/'prepared-owner.jsonl')
    assert len(owners)==324
    owner_groups=defaultdict(list)
    for r in owners:
        owner_groups[r['engine'],r['case'],r['count'],r['code'],r['phase']].append(r)
    retained=[]
    for (engine,case,count,code,phase),rs in sorted(owner_groups.items()):
        assert len(rs)==3 and sorted(r['round'] for r in rs)==[0,1,2]
        record: dict[str, object]=dict(engine=engine,case=case,count=count,code=code,phase=phase,
                    logical_unit_bytes=rs[0]['logical_unit_bytes'],independent_processes=3)
        for name in ['rss_bytes','pss_bytes','live_requested_bytes']:
            xs=[r['allocation'][name] if name=='live_requested_bytes' else r['physical'][name] for r in rs]
            record[name]=dict(mean=st.mean(xs),range=[min(xs),max(xs)],rounds=xs)
        xs=[r['prepare_ns']/1000 for r in rs]
        record['prepare_us']=dict(mean=st.mean(xs),range=[min(xs),max(xs)])
        retained.append(record)
    assert len(retained)==108
    startup=load(out/'process-startup.jsonl')
    assert len(startup)==9
    startup_groups=defaultdict(list)
    for r in startup:startup_groups[r['engine']].append(r)
    startup_out=[]
    for engine,rs in sorted(startup_groups.items()):
        assert sorted(r['round'] for r in rs)==[0,1,2]
        startup_out.append(dict(engine=engine,boundary=rs[0]['boundary'],wall_ms_median=st.median(r['wall_ns'] for r in rs)/1e6,
            wall_ms_range=[min(r['wall_ns'] for r in rs)/1e6,max(r['wall_ns'] for r in rs)/1e6],
            cpu_ms_mean=st.mean(r['cpu_ns'] for r in rs)/1e6))
    result=dict(schema=1,runtime_commit=plan['runtime_commit'],speed=speed,mixed=mixed_out,memory=memory,
                frozen_source_files_verified=len(frozen['files']),
                prepared_owner=retained,process_startup=startup_out,
                initialization=init,pyodide_initialization=load(out/'common-pyodide-initialization.jsonl'),
                coverage=dict(scheduled_commands=len(commands)+126,completed_commands=len(commands)-30+126,failed_commands=observed_failures,skipped_commands=30-observed_failures,
                              unsupported_commands=30,unsupported_rows=sum(s['scheduled_rows'] for s in unsupported),
                              unsupported_cells=[{k:v for k,v in s.items() if k!='prior_error'} for s in unsupported],
                              campaign_commands=len(commands),scheduled_rows=sum(i['expected'] for i in commands)+5430+24,
                              completed_rows=completed_rows+5430+24,
                              prepared_diagnostic_rows=324,process_startup_rows=9,old_initialization_rows=42,pyodide_initialization_rows=3,
                              speed_cells=len(speed),mixed_cells=len(mixed_out),memory_cells=len(memory),
                              required_arms=plan['required_arms'],missing_arms=[],raw_counts=[dict(engine=e,kind=k,rows=v) for (e,k),v in sorted(raw_counts.items())]),
                limitations=plan['limitations'])
    (out/'raw-identity.json').write_text(json.dumps(raw_identity,indent=2)+'\n')
    (out/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result['coverage'],indent=2))
    return result


if __name__=='__main__': reduce(Path(sys.argv[1]),json.loads(Path(sys.argv[2]).read_text()))
