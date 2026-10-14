"""Recompute I evidence using H's unchanged quantile/batch accounting."""
from collections import defaultdict
import gzip
import json
from pathlib import Path
import statistics as stats
from summarize_average_ab import mix_summary
OUT=Path(__file__).resolve().parents[1]/'bench/results/i-stage'
def summarize(path):
    mixed,shapes=defaultdict(list),defaultdict(list)
    count=0; overload=set()
    with gzip.open(path,'rt') as f:
        for line in f:
            r=json.loads(line); count+=1
            if r['kind']=='mixed':
                if r['overloaded']: overload.add((r['build'],r['engine'],r['mix'],r['mode'],r['delay_us'],r['round'],r['sample']))
                if r['sample']>=3: mixed[f"{r['mix']}/{r['mode']}/{r['delay_us']}/{r['build']}/{r['engine']}"].append(r)
            elif r['sample']>=21: shapes[f"{r['case']}:{r['rows']}/{r['mode']}/{r['build']}/{r['engine']}"].append(r)
    identity=json.loads((OUT/(path.name.replace('.jsonl.gz','')+'-identity.json')).read_text())
    assert count==identity['raw_count']
    result={'raw_count':count,'all_overloaded_batches':len(overload),'mixed':{},'shapes':{}}
    for k,rs in sorted(mixed.items()):
        result['mixed'][k]=mix_summary(rs)
        result['mixed'][k]['rounds']=[mix_summary([r for r in rs if r['round']==i]) for i in range(3)]
    def ss(rs):
        return {f'{metric}_mean_us':stats.mean(r[metric+'_ns'] for r in rs)/1000 for metric in ['cpu','wall','prepare','execute']} | {'wall_median_us':stats.median(r['wall_ns'] for r in rs)/1000}
    for k,rs in sorted(shapes.items()):
        assert len(rs)==240
        result['shapes'][k]=ss(rs)|{'rounds':[ss([r for r in rs if r['round']==i]) for i in range(3)]}
    return result
def main():
    result={p.name.replace('.jsonl.gz',''):summarize(p) for p in sorted(OUT.glob('*.jsonl.gz')) if p.name in ['base.jsonl.gz','native-candidate.jsonl.gz','interpreter-candidate.jsonl.gz']}
    memory=defaultdict(list)
    for l in (OUT/'memory.jsonl').read_text().splitlines():
        r=json.loads(l); memory[f"{r['mix']}/{r['engine']}"].append(r)
    assert sum(map(len,memory.values()))==40
    result['memory']={}
    for k,rs in memory.items():
        before=rs[0]['allocation']['live_requested_bytes']
        result['memory'][k]={r['phase']:{'increment_bytes':r['allocation']['live_requested_bytes']-before,'payload_bytes':r['input_payload_bytes'],'rss_bytes':r['physical']['rss_bytes'],'pss_bytes':r['physical']['pss_bytes'],'sampled_peak_bytes':r['physical']['peak_rss_bytes']} for r in rs}
    (OUT/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
    for label,s in result.items():
        if label=='memory': continue
        for k,v in s['mixed'].items(): print(label,k,'CPU',round(v['cpu_us_per_task'],2),'latency',round(v['mean_latency_us'],2),'p99',round(v['short_p99_us'],2),'roundCPU',[round(r['cpu_us_per_task'],2) for r in v['rounds']])
        for k,v in s['shapes'].items(): print(label,k,'CPU',round(v['cpu_mean_us'],2),'prepare',round(v['prepare_mean_us'],2),'execute',round(v['execute_mean_us'],2))
if __name__=='__main__':main()
