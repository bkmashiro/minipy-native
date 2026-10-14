"""Reduce bounded S extension on WSL. Usage: RAW ORIGINAL_SUMMARY OUTPUT.
Missing rows stay missing; retain per-round means instead of synthetic intervals.
"""
from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import statistics as st
import sys
from typing import Any
from s_wsl_extension import schedule, validate, SHAPES, N
from s_wsl_summary import load, by_round, shapes

PHASES=['process_startup_ns','runtime_init_ns','source_prepare_ns','execute_ns','reap_unassigned_ns']


def reduce(raw, original, output):
    root=Path(__file__).resolve().parents[1]
    frozen=json.loads((root/'bench/results/s-wsl/frozen-source.json').read_text())
    for p,digest in frozen['files'].items():
        path=root/('bench/results/s-wsl/frozen-interpreter_ab.rs' if p=='bench/src/bin/interpreter_ab.rs' else p)
        assert hashlib.sha256(path.read_bytes()).hexdigest()==digest,(p,'frozen drift')
    grouped=defaultdict(list)
    coverage={}
    unsupported=[]
    raw_identity={}
    for mode in ['first','reference','curve']:
        identity=json.loads((raw/(mode+'-identity.json')).read_text())
        for name,meta in identity['files'].items():
            path=Path(name)
            if path==root/'bench/src/bin/assembly_ab.rs' and hashlib.sha256(path.read_bytes()).hexdigest()!=meta['sha256']:
                path=root/'bench/results/s-wsl/measured-assembly_ab.rs'
            assert path.stat().st_size==meta['bytes'] and hashlib.sha256(path.read_bytes()).hexdigest()==meta['sha256'],name
        work=schedule(mode)
        assert json.loads((raw/(mode+'-commands.json')).read_text())==work
        status=json.loads((raw/(mode+'-status.json')).read_text())
        assert len(status)==len(work) and {s['key'] for s in status}=={i['key'] for i in work}
        states={s['key']:s for s in status}
        assert all(s['state'] in ['completed','unsupported'] for s in status),[s for s in status if s['state'] not in ['completed','unsupported']]
        total=0
        for item in work:
            s=states[item['key']]
            if s['state']=='unsupported':
                assert 'unsupported' in item and item['tags']['engine'].startswith('pysolate-')
                unsupported.append(dict(cohort=mode,**item['tags'],**item['unsupported']))
                continue
            path=raw/(item['key']+'.jsonl.gz')
            rs=load(path)
            raw_identity[path.name]=dict(sha256=hashlib.sha256(path.read_bytes()).hexdigest(),bytes=path.stat().st_size)
            selected=validate(rs,item)
            total+=len(selected)
            for r in selected:
                assert r['round']==item['tags']['round'] and r['command_key']==item['key']
                if mode!='first' and r['sample']<item['discard']:continue
                key=(mode,r['engine'],r.get('mode','first-request'),r['case'],r['rows'])
                grouped[key].append(r)
        missing=sum(i['expected'] for i in work if 'unsupported' in i)
        assert total+missing==sum(i['expected'] for i in work)
        coverage[mode]=dict(scheduled_commands=len(work),**dict(Counter(s['state'] for s in status)),
                            scheduled_rows=sum(i['expected'] for i in work),completed_rows=total,unsupported_rows=missing)
    original=json.loads(original.read_text())
    # Original and extension don't overlap. Per-point cohort travels to plot.
    curve=[dict(r,cohort='original') for r in original['speed']]
    reference=[]
    first=[]
    for (cohort,e,m,c,n),rs in sorted(grouped.items()):
        if cohort=='first':
            assert len(rs)==3 and sorted(r['round'] for r in rs)==[0,1,2]
            metrics: dict[str, dict[str, Any]]={k:dict(mean_ms=st.mean(r[k] for r in rs)/1e6,
                       median_ms=st.median(r[k] for r in rs)/1e6,
                       range_ms=[min(r[k] for r in rs)/1e6,max(r[k] for r in rs)/1e6],
                       rounds_ms=[r[k]/1e6 for r in sorted(rs,key=lambda r:r['round'])]) for k in PHASES+['wall_ns','cpu_ns']}
            assert abs(sum(metrics[k]['mean_ms'] for k in PHASES)-metrics['wall_ns']['mean_ms'])<1e-8
            first.append(dict(engine=e,case=c,rows=n,independent_processes=3,metrics=metrics,
                              prepare_boundary=rs[0]['prepare_boundary'],execute_boundary=rs[0]['execute_boundary']))
        else:
            per_round=40 if e.startswith('pypy') else 20 if e.startswith('pyodide') else 10 if e.startswith('pysolate') else 30
            assert len(rs)==per_round*3 and len({(r['round'],r['sample']) for r in rs})==len(rs)
            result=dict(engine=e,mode=m,case=c,rows=n,cohort='extension',**by_round(rs,shapes))
            if cohort=='curve':curve.append(result)
            else:
                result['boundary']='kernel-only' if m=='kernel' else 'full-request'
                result['kernel_code_bytes']=rs[0].get('kernel_code_bytes') if e=='assembly' else None
                reference.append(result)
    assert len(curve)==262 and len(reference)==56 and len(first)==24
    assert len({(r['engine'],r['mode'],r['case'],r['rows']) for r in curve})==len(curve)
    assert {(r['case'],r['rows']) for r in curve}==set(SHAPES)
    # Compute only observed crossing brackets, no smooth curve extrapolation.
    crossings=[]
    for case in ['weighted','stateful']:
        for mode in ['cold','warm']:
            points=[]
            for n in N:
                a=next(r for r in curve if (r['engine'],r['mode'],r['case'],r['rows'])==('native',mode,case,n))
                b=next(r for r in curve if (r['engine'],r['mode'],r['case'],r['rows'])==('interpreter',mode,case,n))
                points.append(dict(rows=n,native_us=a['wall_mean_us'],interpreter_us=b['wall_mean_us'],
                                   native_faster=a['wall_mean_us']<b['wall_mean_us'],cohort=a['cohort']))
            brackets=[[a['rows'],b['rows']] for a,b in zip(points,points[1:]) if a['native_faster']!=b['native_faster']]
            crossings.append(dict(case=case,mode=mode,points=points,observed_crossing_brackets=brackets))
    result=dict(runtime_commit=frozen['runtime_commit'],curve=curve,reference=reference,first=first,
                coverage=coverage,unsupported=unsupported,crossings=crossings,
                source_binding='Frozen runtime/fixtures plus archived exact original interpreter_ab; separately hashed first-request entry and assembly extension',
                first_boundary='single fresh process; launch + runtime/harness init + source prepare + execution + response/reap/unassigned; each raw sums exactly',
                curve_cohorts='Original successes retained; only absent shapes in extension; cohorts never pooled at a shared point')
    output.mkdir(parents=True,exist_ok=True)
    (output/'extension-summary.json').write_text(json.dumps(result,indent=2)+'\n')
    (output/'extension-raw-identity.json').write_text(json.dumps(raw_identity,indent=2)+'\n')
    print(json.dumps(dict(coverage=coverage,crossings=crossings),indent=2))
    return result


if __name__=='__main__':reduce(Path(sys.argv[1]),Path(sys.argv[2]),Path(sys.argv[3]))
