#!/usr/bin/env python3
"""One ABBA pair per case/backend plus A/A control; replayable raw and reduction."""
import csv
import fcntl
import json
from pathlib import Path
import statistics
import subprocess
import sys

root = Path(__file__).resolve().parents[1]
out = root / 'bench/results/cw'
if '--reduce' not in sys.argv:
  with open('/Users/yuzhe/.hermes/cache/scratch/minipy-perf-build.lock', 'a') as lock:
    fcntl.flock(lock, fcntl.LOCK_EX)
    load = {}
    with (out/'speed.csv').open('w') as f:
        f.write('backend,case,phase,position,arm,iteration,ns,calls,bytes\n')
        for backend in ['tape','native']:
            for case in ['no_int','short','dense']:
                for phase, arms in [('ABBA','ABBA'), ('AA','AA')]:
                    for position, arm in enumerate(arms):
                        key = f'{backend}/{case}/{phase}/{position}'
                        ps = subprocess.check_output(['ps','-Ao','pid,pcpu,comm','-r'],text=True).splitlines()
                        load[key] = ps[:9]
                        binary = root/'target/release/examples'/f'cw_{arm}_speed'
                        data = subprocess.check_output([str(binary),backend,case,'64'],text=True)
                        for row in csv.reader(data.splitlines()):
                            assert row[:2] == [case,str(backend == 'native').lower()]
                            assert row[4:] == ['0','0']
                            f.write(f'{backend},{case},{phase},{position},{arm},{",".join(row[2:])}\n')
    (out/'load.json').write_text(json.dumps(load,indent=2)+'\n')
rows = list(csv.DictReader((out/'speed.csv').open()))
assert len(rows) == 2*3*6*64
result = {}
for backend in ['tape','native']:
    for case in ['no_int','short','dense']:
        groups = {(phase,pos): statistics.median(int(r['ns']) for r in rows if r['backend']==backend and r['case']==case and r['phase']==phase and int(r['position'])==pos) for phase, n in [('ABBA',4),('AA',2)] for pos in range(n)}
        result[f'{backend}/{case}'] = dict(medians_ns={f'{p}/{i}':v for (p,i),v in groups.items()}, BA=(groups['ABBA',1]+groups['ABBA',2])/(groups['ABBA',0]+groups['ABBA',3]), AA=groups['AA',1]/groups['AA',0])
alloc = {a:list(csv.DictReader((out/f'{a}_alloc.csv').open())) for a in 'AB'}
for backend in ['tape','native']:
    for case in ['no_int','short','dense']:
        x = {}
        for arm in 'AB':
            rr=[r for r in alloc[arm] if r['case']==case and r['native']==str(backend=='native').lower()]
            assert len(rr)==3
            x[arm]={k:statistics.median(int(r[k]) for r in rr) for k in ['calls','bytes']}
        result[f'{backend}/{case}']['allocation']=x
(out/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
