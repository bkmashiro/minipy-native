"""L timing-boundary accounting, not an estimate of unmeasured install time."""
import gzip
import json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'bench/results/l-final'


def main():
    result=[]
    for name in ['speed','cpython','pyodide','common-primary','common-cpython','common-pyodide',
                 'assembly','assembly-v2','assembly-full','scalar-recheck']:
        path=OUT/(name+'.jsonl.gz')
        if not path.exists():continue
        with gzip.open(path,'rt') as f:rows=[json.loads(l) for l in f]
        singles=[r for r in rows if r['kind']!='mixed']
        batches={}
        for r in rows:
            if r['kind']=='mixed':
                key=tuple(r.get(k) for k in ['engine','mix','mode','delay_us','inflight','round','sample'])
                batches.setdefault(key,r)
        wall=sum(r['wall_ns'] for r in singles)+sum(max(100_000_000,r['batch_wall_ns']) for r in batches.values())
        cpu=sum(r['cpu_ns'] for r in singles)+sum(r['batch_cpu_ns'] for r in batches.values())
        result.append({'cohort':name,'raw_observations':len(rows),'single_requests':len(singles),
                       'mixed_windows':len(batches),'accounted_guest_window_wall_s':wall/1e9,
                       'accounted_guest_cpu_s':cpu/1e9})
    payload={'scope':'Sum of directly measured serial request boundaries plus nominal-or-draining mixed windows, including warmups; not entire subprocess/SSH duration. Historical assembly/refinement/recheck observations counted separately, never pooled for performance ranking.',
             'cohorts':result,'accounted_guest_window_wall_s':sum(r['accounted_guest_window_wall_s'] for r in result),
             'not_measured':'Component-level elapsed time for reading/designing harness, initial installs, SSH setup/serialization, analysis, report/review and compilation; do not assign the remaining task elapsed time to CPU speed or installation'}
    (OUT/'timing-accounting.json').write_text(json.dumps(payload,indent=2)+'\n')
    print(json.dumps(payload,indent=2))


if __name__=='__main__':main()
