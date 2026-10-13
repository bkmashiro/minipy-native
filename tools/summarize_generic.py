"""Validate the separate generic-helper rerun, then aggregate measured totals."""
import collections
import csv
from pathlib import Path
import statistics

out = Path(__file__).resolve().parents[1] / 'bench/results'
summary = []
for platform in ['macos', 'linux']:
    path = out / f'generic-{platform}.csv'
    if not path.exists():
        print(f'{platform}: pending (no raw file)')
        continue
    with path.open() as f:
        rows = list(csv.DictReader(line for line in f if not line.startswith('#')))
    assert len(rows) == 480, (path, len(rows))
    groups = collections.defaultdict(list)
    seen = set()
    for r in rows:
        n = int(r['rows'])
        receipt = 17 + sum(((i * 37) % 1000) * ((i % 9) - 2) for i in range(n)
                           if (i % 9) - 2 > 0 and (i * 37) % 1000 <= 700)
        assert r['case'] == 'weighted_scan' and n in [0, 100, 10000]
        assert int(r['receipt']) == receipt
        assert int(r['total_ns']) > 0
        key = tuple(r[k] for k in ['case', 'rows', 'engine', 'mode', 'chunk', 'sample'])
        assert key not in seen, (path, key)
        seen.add(key)
        groups[key[:-1]].append(r)
    assert len(groups) == 24
    for key, samples in groups.items():
        assert {int(r['sample']) for r in samples} == set(range(20))
        result = dict(zip(['case', 'rows', 'engine', 'mode', 'chunk'], key))
        result.update(platform=platform, samples=len(samples))
        for column in ['total_ns', 'compile_ns', 'conversion_ns', 'execute_cleanup_ns']:
            result[column.replace('_ns', '_median_us')] = round(statistics.median(int(r[column]) for r in samples) / 1000, 3)
        summary.append(result)
        if key[1] == '10000':
            print(result)
    print(f'{platform}: verified {len(rows)} samples / {len(groups)} groups')
with (out / 'generic-summary.csv').open('w') as f:
    writer = csv.DictWriter(f, fieldnames=list(summary[0]), lineterminator='\n')
    writer.writeheader()
    writer.writerows(summary)
