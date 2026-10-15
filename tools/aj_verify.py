"""Recompute and check the committed AJ batch summaries without running a benchmark."""
import csv
import json
from pathlib import Path
import statistics

root = Path(__file__).resolve().parents[1] / 'evidence/aj-bounded'
counts = {}
for phase in ['candidate', 'final']:
    directory = root / phase
    rows = []
    for file in sorted(directory.glob('[0-3]-*.csv')):
        pair, snapshot, probe, driver = file.stem.split('-')
        batch = list(csv.DictReader(file.open()))
        assert len(batch) == (99 if probe == 'json' else 12)
        rows.extend(dict(r, pair=int(pair), snapshot=snapshot, probe=probe, driver=driver) for r in batch)
    assert len(rows) == 984
    counts[phase] = len(rows)
    summary = []
    for key in sorted({(r['probe'],r['driver'],r['workload'],r['arm'],r['lifecycle']) for r in rows}):
        rs = [r for r in rows if (r['probe'],r['driver'],r['workload'],r['arm'],r['lifecycle']) == key]
        assert len({int(r['checksum']) / int(r['iterations']) for r in rs}) == 1
        times = {}
        for arm in ['base','candidate']:
            times[arm] = []
            for pair in range(4):
                selected = [r for r in rs if r['snapshot']==arm and r['pair']==pair]
                times[arm].append(sum(int(r['total_ns']) for r in selected) / sum(int(r['iterations']) for r in selected) / 1000)
        old,new = map(statistics.median,[times['base'],times['candidate']])
        summary.append(dict(zip(['probe','driver','workload','engine','lifecycle'],key),base_us=old,candidate_us=new,
                            change_pct=(new/old-1)*100,pair_change_pct=[(b/a-1)*100 for a,b in zip(times['base'],times['candidate'])],process_means_us=times))
    assert summary == json.loads((directory/'summary.json').read_text())

rows=[]
for file in sorted((root/'default-control').glob('[0-3]-*.csv')):
    pair,arm=file.stem.split('-',1)
    rows.extend(dict(r,pair=int(pair),snapshot=arm) for r in csv.DictReader(file.open()))
assert len(rows)==144
counts['default_control']=len(rows)
for summary in json.loads((root/'default-control/summary.json').read_text()):
    rs=[r for r in rows if (r['workload'],r['arm'],r['lifecycle'])==(summary['workload'],summary['engine'],summary['lifecycle'])]
    assert len({int(r['checksum'])/int(r['iterations']) for r in rs})==1
    for arm,expected in summary['median_us'].items():
        times=[int(r['total_ns'])/int(r['iterations'])/1000 for r in rs if r['snapshot']==arm]
        assert statistics.median(times)==expected
        assert {int(r['prepared_bytes']) for r in rs if r['snapshot']==arm}=={summary['tape_bytes'][arm]}
        assert {int(r['code_bytes']) for r in rs if r['snapshot']==arm}=={summary['native_bytes'][arm]}

rows=[]
for file in sorted((root/'http').glob('process-*.csv')):
    rows.extend(dict(r,process=file.stem) for r in csv.DictReader(file.open()))
assert len(rows)==72
counts['http']=len(rows)
summary=[]
for key in sorted({(r['syntax'],r['engine'],r['lifecycle']) for r in rows}):
    rs=[r for r in rows if (r['syntax'],r['engine'],r['lifecycle'])==key]
    assert len(rs)==9
    assert all(int(r['checksum'])==115*int(r['iterations']) and int(r['responses'])==2*(int(r['iterations'])+1) and int(r['connections'])==1 for r in rs)
    means=[sum(int(r['total_ns']) for r in rs if r['process']==p)/sum(int(r['iterations']) for r in rs if r['process']==p)/1000 for p in sorted({r['process'] for r in rs})]
    summary.append(dict(zip(['syntax','engine','lifecycle'],key),median_process_us=statistics.median(means),process_means_us=means,prepared_bytes=int(rs[0]['prepared_bytes']),code_bytes=int(rs[0]['code_bytes'])))
assert summary==json.loads((root/'http/summary.json').read_text())
alloc=list(csv.DictReader((root/'allocation.csv').open()))
assert len(alloc)==33 and all(r['mode']=='allocation' for r in alloc)
assert all(r['allocation_calls'] and r['allocated_bytes'] for r in alloc)
counts['allocation']=len(alloc)
print(json.dumps(dict(batch_rows=counts,total_batch_rows=sum(counts.values()),summary_check='exact match')))
