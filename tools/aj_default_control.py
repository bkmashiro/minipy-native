"""Default-feature pre-AH/current and opt-in final small regression controls."""
import csv
import hashlib
import io
import json
import os
from pathlib import Path
import statistics
import subprocess

out = Path('evidence/aj-bounded/default-control')
out.mkdir(parents=True, exist_ok=True)
rows = []
hashes = {}
orders = [('pre-ah', 'default', 'final'), ('final', 'default', 'pre-ah'),
          ('final', 'default', 'pre-ah'), ('pre-ah', 'default', 'final')]
for pair, order in enumerate(orders):
    for arm in order:
        binary = Path(f'../aj-{arm}-fastpath').resolve()
        hashes[arm] = hashlib.sha256(binary.read_bytes()).hexdigest()
        env = dict(os.environ)
        env.pop('MINIPY_IO_DRIVER', None)
        text = subprocess.check_output([str(binary)], text=True, env=env)
        (out / f'{pair}-{arm}.csv').write_text(text)
        batch = list(csv.DictReader(io.StringIO(text)))
        assert len(batch) == 12
        rows.extend(dict(r, snapshot=arm, pair=pair) for r in batch)
assert len(rows) == 144
summary = []
for key in sorted({(r['workload'], r['arm'], r['lifecycle']) for r in rows}):
    selected = [r for r in rows if (r['workload'], r['arm'], r['lifecycle']) == key]
    assert len({int(r['checksum']) / int(r['iterations']) for r in selected}) == 1
    times = {arm: [int(r['total_ns']) / int(r['iterations']) / 1000
                   for r in selected if r['snapshot'] == arm] for arm in orders[0]}
    assert all(len(v) == 4 for v in times.values())
    summary.append(dict(zip(['workload', 'engine', 'lifecycle'], key),
                        median_us={k: statistics.median(v) for k,v in times.items()},
                        process_means_us=times,
                        tape_bytes={arm: int(next(r['prepared_bytes'] for r in selected if r['snapshot']==arm)) for arm in orders[0]},
                        native_bytes={arm: int(next(r['code_bytes'] for r in selected if r['snapshot']==arm)) for arm in orders[0]}))
(out / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
(out / 'hashes.json').write_text(json.dumps(hashes, indent=2) + '\n')
print(f'{len(rows)} rows, {len(summary)} cells, checksums matched')
