"""Independent L recomputation; deliberately does not import summary reducer."""
import gzip
import json
import math
from pathlib import Path
import statistics
import xml.etree.ElementTree as ET
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'bench/results/l-final'


def load(name):
    with gzip.open(OUT/(name+'.jsonl.gz'),'rt') as f:return [json.loads(l) for l in f]


def near(a,b):
    assert math.isclose(a,b,rel_tol=1e-12,abs_tol=1e-9),(a,b)


def check_group(raw,summary,shape_cut,mix_cut,secondary=False):
    checks=0
    for r in summary['shapes']:
        rows=[v for v in raw if v['kind']!='mixed' and v['engine']==r['engine'] and v['case']==r['case'] and v['rows']==r['rows'] and (('warm' if v['engine'].endswith('prepared') else 'cold') if secondary else v['mode'])==r['mode'] and v['sample']>=shape_cut]
        assert len(rows)==r['requests'] and all(v['completed'] for v in rows)
        for key in ['wall','cpu','prepare','execute','conversion']:
            values=[v[key+'_ns'] for v in rows if key+'_ns' in v]
            actual=r.get(key+'_mean_us')
            if values:near(statistics.mean(values)/1000,actual)
            else:assert actual is None
            checks+=1
    for r in summary['mixed']:
        rows=[v for v in raw if v['kind']=='mixed' and v['engine']==r['engine'] and v['mix']==r['mix'] and v['delay_us']==r['delay_us'] and v['inflight']==r['inflight'] and (('warm' if v['engine'].endswith('prepared') else 'cold') if secondary else v['mode'])==r['mode'] and v['sample']>=mix_cut]
        assert len(rows)==r['requests'] and all(v['completed'] for v in rows)
        batches={}
        for v in rows:batches.setdefault((v['round'],v['sample']),v)
        assert len(rows)==40*len(batches)
        near(sum(v['batch_cpu_ns'] for v in batches.values())/len(rows)/1000,r['cpu_us_per_task'])
        near(statistics.mean(v['latency_ns'] for v in rows)/1000,r['mean_latency_us'])
        short=sorted(v['latency_ns'] for v in rows if v['rows']<=100)
        for p in [50,95,99]:near(short[math.ceil(len(short)*p/100)-1]/1000,r[f'short_p{p}_us'])
        assert sum(v['overloaded'] for v in batches.values())==r['overloaded_windows']
        near(len(rows)*1e9/sum(max(100000000,v['batch_wall_ns']) for v in batches.values()),r['finite_window_requests_per_s'])
        checks+=7
    return checks


def main():
    s=json.loads((OUT/'summary.json').read_text());checks=0;counts={}
    mapping=[('speed',s['primary'],21,2,False),('cpython',s['secondary']['cpython'],1,1,True),
             ('pyodide',s['secondary']['pyodide'],1,1,True),('common-primary',s['common']['primary'],11,1,False),
             ('common-cpython',s['common']['cpython'],1,1,True),('common-pyodide',s['common']['pyodide'],1,1,True)]
    for name,summary,sc,mc,secondary in mapping:
        raw=load(name);counts[name]=len(raw);checks+=check_group(raw,summary,sc,mc,secondary)
    raw=[r for r in load('assembly-v2') if r['mode']=='kernel']+load('assembly-full')
    for r in s['assembly']['shapes']:
        rows=[v for v in raw if all(v[k]==r[k] for k in ['engine','case','mode','rows','delay_us']) and v['sample']>=21]
        assert len(rows)==240
        near(statistics.mean(v['wall_ns'] for v in rows)/1000,r['wall_mean_us'])
        near(statistics.mean(v['cpu_ns'] for v in rows)/1000,r['cpu_mean_us']);checks+=2
    for name,summary in [('memory',s['memory']),('cpython-memory',s['secondary_memory']),('pyodide-memory',s['secondary_memory']),('assembly-full-memory',s['assembly_memory'])]:
        raw=[json.loads(l) for l in (OUT/(name+'.jsonl')).read_text().splitlines()]
        for r in summary:
            rows=[v for v in raw if (v['engine'],v['code'],v['count'],v['phase'])==(r['engine'],r['code'],r['tasks'],r['phase'])]
            if not rows:continue
            assert len(rows)==3
            for key in ['rss_bytes','pss_bytes']:near(statistics.mean(v['physical'][key] for v in rows),r[key]['mean']);checks+=1
    for name in ['assembly','assembly-v2','assembly-full','scalar-recheck']:
        raw=load(name);counts[name]=len(raw);assert all(r['completed'] for r in raw)
    assert sum(counts.values())==145860
    for p in (ROOT/'docs/figures').glob('benchmark-*.svg'):
        tree=ET.parse(p);assert tree.getroot().attrib.get('viewBox')
        assert len(tree.findall('.//{http://www.w3.org/2000/svg}text'))>=20
    result={'independent_numeric_checks':checks,'retained_raw_counts':counts,'retained_speed_observations':sum(counts.values()),
            'result':'pass','scope':'means, whole batch CPU once, nearest-rank tails, overload, finite-window throughput, physical memory and corrected assembly boundaries independently recomputed; SVG XML parses'}
    (OUT/'verification.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))


if __name__=='__main__':main()
