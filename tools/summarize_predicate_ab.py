"""Verify and reduce F raw data; stdout is reproducible summary JSON."""
from collections import defaultdict
import gzip
import json
from pathlib import Path
from statistics import median
import sys

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'bench/results/f-predicate'
METRICS = ['wall_ns', 'cpu_ns', 'compile_ns', 'code_bytes']


def summarize(rows):
    return {m: median(r[m] for r in rows) for m in METRICS}


def main():
    followup = sys.argv[1:] == ['followup']
    assert not sys.argv[1:] or followup
    filename = 'followup.jsonl.gz' if followup else 'linux.jsonl.gz'
    last, stable_start = (1000, 501) if followup else (100, 6)
    group_count = 36 if followup else 60
    stable_count = last - stable_start + 1
    with gzip.open(OUT / filename, 'rt') as f:
        rows = [json.loads(line) for line in f]
    assert len(rows) == (36036 if followup else 6072)
    samples = [r for r in rows if r['kind'] == 'sample']
    inventories = [r for r in rows if r['kind'] == 'inventory']
    keys = [(r['round'], r['variant'], r['case'], r['rows'], r['mode'], r['sample']) for r in samples]
    assert len(keys) == len(set(keys)) == group_count * (last + 1)
    assert all(r['ok'] and r['wall_ns'] > 0 and r['cpu_ns'] > 0 for r in samples)
    groups = defaultdict(list)
    pooled = defaultdict(list)
    for r in samples:
        groups[(r['round'], r['variant'], r['case'], r['rows'], r['mode'])].append(r)
        if r['sample'] >= stable_start:
            pooled[(r['variant'], r['case'], r['rows'], r['mode'])].append(r)
    assert len(groups) == group_count and len(pooled) == group_count // 3
    output = {'rows': len(rows), 'inventory': inventories, 'rounds': [], 'pooled': []}
    for key, batch in sorted(groups.items()):
        assert sorted(r['sample'] for r in batch) == list(range(last + 1))
        stable = [r for r in batch if r['sample'] >= stable_start]
        assert len(stable) == stable_count
        assert len({r['code_bytes'] for r in batch}) == 1
        output['rounds'].append(dict(zip(['round', 'variant', 'case', 'rows', 'mode'], key)) | {
            'n': len(stable), 'medians': summarize(stable),
            'first': summarize([r for r in batch if r['sample'] == 0]),
            'transition': summarize([r for r in batch if 1 <= r['sample'] < stable_start]),
        })
    for key, batch in sorted(pooled.items()):
        assert len(batch) == stable_count * 3
        output['pooled'].append(dict(zip(['variant', 'case', 'rows', 'mode'], key)) | {'n': len(batch), 'medians': summarize(batch)})
    inventory_count = 0 if followup else 12
    assert len(inventories) == inventory_count
    assert len({(r['variant'], r['case'], r['when']) for r in inventories}) == inventory_count
    for r in samples:
        inv = [x for x in inventories if x['variant'] == r['variant'] and x['case'] == r['case']]
        assert all(x['code_bytes'] == r['code_bytes'] for x in inv)
    output['round_deltas_percent'] = []
    for b in output['rounds']:
        if b['variant'] != 'baseline':
            continue
        c = next(r for r in output['rounds'] if r['variant'] == 'candidate' and all(r[k] == b[k] for k in ['round', 'case', 'rows', 'mode']))
        output['round_deltas_percent'].append({k: b[k] for k in ['round', 'case', 'rows', 'mode']} | {
            m: (c['medians'][m] / b['medians'][m] - 1) * 100 for m in ['wall_ns', 'cpu_ns']
        })
    print(json.dumps(output, indent=2))


if __name__ == '__main__':
    main()
