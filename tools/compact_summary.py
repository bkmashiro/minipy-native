"""Recompute N raw using unchanged H/K CPU and nearest-rank accounting."""
from collections import defaultdict
import gzip
import json
from pathlib import Path
import statistics as stats
from summarize_average_ab import mix_summary

OUT = Path(__file__).resolve().parents[1] / 'bench/results/n-compact'

def read_gz(name):
    with gzip.open(OUT/name, 'rt') as f:
        return [json.loads(line) for line in f]

def mean_metrics(rows):
    return {m+'_us': stats.mean(r[m+'_ns'] for r in rows)/1000
            for m in ['cpu','wall','prepare','execute']}

def summarize():
    raw=read_gz('linux.jsonl.gz')
    ident=json.loads((OUT/'linux-identity.json').read_text())
    assert len(raw)==ident['raw_count']==20560
    assert all(r['engine']=='interpreter' and r['completed'] for r in raw)
    mixed,shapes=defaultdict(list),defaultdict(list)
    overload=set()
    for r in raw:
        if r['kind']=='mixed':
            if r['overloaded']: overload.add((r['build'],r['round'],r['mix'],r['mode'],r['delay_us'],r['sample']))
            if r['sample']>=2: mixed[f"{r['mix']}/{r['mode']}/{r['delay_us']}/{r['build']}"].append(r)
        elif r['sample']>=11: shapes[f"{r['case']}:{r['rows']}/{r['mode']}/{r['build']}"].append(r)
    assert len(mixed)==12 and len(shapes)==20
    result={'raw_count':len(raw),'overloaded_windows':len(overload),'mixed':{},'shapes':{},'frontend':{},'memory':{}}
    for category,groups,fn,expected in [('mixed',mixed,mix_summary,1120),('shapes',shapes,mean_metrics,120)]:
        for k,rs in sorted(groups.items()):
            assert len(rs)==expected
            result[category][k]=fn(rs)|{'rounds':[fn([r for r in rs if r['round']==i]) for i in range(4)]}
    front=read_gz('frontend-macos.jsonl.gz')
    assert len(front)==11616 and all(r['completed'] for r in front)
    groups=defaultdict(list)
    for r in front:
        if r['sample']>=21: groups[f"{r['comparison']}/{r['engine']}/{r['case']}:{r['rows']}/{r['build']}"].append(r)
    assert len(groups)==24
    for k,rs in sorted(groups.items()):
        assert len(rs)==400
        result['frontend'][k]=mean_metrics(rs)|{'rounds':[mean_metrics([r for r in rs if r['round']==i]) for i in range(4)]}
    memory=[json.loads(l) for l in (OUT/'memory.jsonl').read_text().splitlines()]
    assert len(memory)==ident['memory_count']==96
    cohorts=defaultdict(dict)
    for r in memory:
        key=(r['build'],r['round'],r.get('mix',r.get('case')),r.get('rows',0),r['code'])
        assert r['phase'] not in cohorts[key]
        cohorts[key][r['phase']]=r
    assert len(cohorts)==24
    memory_groups=defaultdict(list)
    for (build,pair,case,n,code),phases in cohorts.items():
        assert set(phases)=={'startup','load_wait','record_wait','reclaimed'}
        start=phases['startup']; before=start['allocation']['live_requested_bytes']
        k=f'{case}:{n}/{code}/{build}'
        metrics={}
        for phase,r in phases.items():
            a=r['allocation']; count=r['count']
            metrics[phase]={'live_requested_bytes':a['live_requested_bytes'],
                            'live_increment_bytes':a['live_requested_bytes']-before,
                            'overhead_per_task_bytes':(a['live_requested_bytes']-before-r['input_payload_bytes'])/count,
                            'pss_bytes':r['physical']['pss_bytes'],
                            'rss_bytes':r['physical']['rss_bytes'],
                            'sampled_peak_rss_bytes':r['physical']['peak_rss_bytes'],
                            'allocation_calls_since_start':a['allocation_calls']-start['allocation']['allocation_calls']}
        memory_groups[k].append(metrics)
    for k,rs in sorted(memory_groups.items()):
        assert len(rs)==2
        result['memory'][k]={phase:{m:stats.mean(r[phase][m] for r in rs) for m in rs[0][phase]} for phase in rs[0]}|{'rounds':rs}
    return result

if __name__=='__main__':
    result=summarize()
    (OUT/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
    print('raw',result['raw_count'],'overload',result['overloaded_windows'])
    for category in ['mixed','shapes']:
        m='cpu_us_per_task' if category=='mixed' else 'cpu_us'
        for k,b in result[category].items():
            if not k.endswith('/base'): continue
            c=result[category][k.removesuffix('/base')+'/candidate']
            print(k.removesuffix('/base'),round(b[m],3),'->',round(c[m],3), 'change%',round(100*(c[m]/b[m]-1),2),'pairs',[round(100*(y[m]/x[m]-1),2) for x,y in zip(b['rounds'],c['rounds'])])
            if category=='mixed': print('short p95/p99',[(round(b[m],2),round(c[m],2)) for m in ['short_p95_us','short_p99_us']])
    for comparison,a,b in [('bitset','baseline','bitset'),('container','bitset','frontend')]:
        for engine in ['interpreter','native']:
            for case in ['scalar:0','weighted:100','stateful:100']:
                x=result['frontend'][f'{comparison}/{engine}/{case}/{a}']; y=result['frontend'][f'{comparison}/{engine}/{case}/{b}']
                print('frontend',comparison,engine,case,'prepare',round(x['prepare_us'],3),'->',round(y['prepare_us'],3),'cpu',round(x['cpu_us'],3),'->',round(y['cpu_us'],3))
    for k,b in result['memory'].items():
        if not k.endswith('/base'): continue
        c=result['memory'][k.removesuffix('/base')+'/candidate']
        print('memory',k.removesuffix('/base'),[(phase,round(b[phase]['overhead_per_task_bytes'],2),round(c[phase]['overhead_per_task_bytes'],2),b[phase]['pss_bytes'],c[phase]['pss_bytes']) for phase in ['load_wait','record_wait','reclaimed']])
