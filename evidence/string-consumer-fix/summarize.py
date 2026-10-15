"""Recompute per-process medians; do not pool process drift away."""
import gzip
import json
import pathlib
import statistics
from collections import defaultdict

root = pathlib.Path(__file__).parent
result = {}
for name, expected in [('requests', 4848), ('requests-confirm', 9696)]:
    with gzip.open(root / (name + '.jsonl.gz'), 'rt') as f:
        rows = [json.loads(line) for line in f]
    assert len(rows) == expected
    groups = defaultdict(list)
    for r in rows:
        if r['sample'] > 0:
            groups[(r['run'], r['arm'], r['backend'], r['case'])].append(r['ns'])
    cells = []
    for (run, arm, backend, case), values in sorted(groups.items()):
        assert len(values) == 100
        cells.append(dict(run=run, arm=arm, backend=backend, case=case,
                          median_ns=statistics.median(values)))
    result[name] = cells
print(json.dumps(result, indent=2))
