"""Validate every raw sample and aggregate in code; no hand-counted benchmark summaries."""
import collections
import csv
import math
from pathlib import Path
import statistics

root = Path(__file__).resolve().parents[1]
out = root / 'bench/results'
rows_out = []
for path in sorted(out / f'{platform}-{opt}.csv' for platform in ['linux', 'macos'] for opt in ['speed', 'none']):
    with path.open() as f:
        rows = list(csv.DictReader(line for line in f if not line.startswith('#')))
    assert len(rows) == 1494, (path, len(rows))
    seen = set()
    groups = collections.defaultdict(list)
    for r in rows:
        r['engine'] = {'monty_helper': 'monty_fixture_helper', 'host': 'host_fixture'}.get(r['engine'], r['engine'])
        key = tuple(r[k] for k in ['case', 'rows', 'engine', 'mode', 'chunk', 'sample'])
        assert key not in seen, (path, key)
        seen.add(key)
        assert int(r['total_ns']) > 0
        groups[key[:-1]].append(r)
    assert len(groups) == 84, (path, len(groups))
    for key, samples in groups.items():
        expected_n = 8 if key[3] == 'cancellation' else 1 if key[3] == 'mixed_heavy' else 20
        assert len(samples) == expected_n, (path, key, len(samples))
        values = sorted(int(r['total_ns']) for r in samples)
        row = dict(zip(['case', 'rows', 'engine', 'mode', 'chunk'], key))
        row.update(platform_opt=path.stem, samples=len(samples),
                   total_median_us=round(statistics.median(values)/1000, 3),
                   total_max_us=round(max(values)/1000, 3),
                   total_sample_p95_us=round(values[math.ceil(len(values)*.95)-1]/1000, 3),
                   compile_median_us=round(statistics.median(int(r['compile_ns']) for r in samples)/1000,3),
                   conversion_median_us=round(statistics.median(int(r['conversion_ns']) for r in samples)/1000,3),
                   execution_cleanup_median_us=round(statistics.median(int(r['execute_cleanup_ns']) for r in samples)/1000,3),
                   code_bytes=max(int(r['code_bytes']) for r in samples))
        rows_out.append(row)
assert len(rows_out) == 336, len(rows_out)
with (out / 'summary.csv').open('w') as f:
    writer = csv.DictWriter(f, fieldnames=list(rows_out[0]), lineterminator='\n')
    writer.writeheader()
    writer.writerows(rows_out)
print('Verified 5,976 unique raw samples across 4 runs; wrote 336 summary groups.')
for r in rows_out:
    if r['platform_opt'] == 'linux-speed' and r['rows'] == '10000' and r['chunk'] != '1':
        print(r)
