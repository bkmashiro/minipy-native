import gzip
import json
from pathlib import Path
from statistics import median

rows = [json.loads(line) for line in gzip.open(Path(__file__).with_name('requests.jsonl.gz'), 'rt')]
assert len(rows) == 4848
for inputs in [0, 2, 32]:
    for backend in ['tape', 'native']:
        values = []
        for run in range(8):
            group = [r for r in rows if r['inputs'] == inputs and r['backend'] == backend and r['run'] == run]
            assert len(group) == 101 and sorted(r['sample'] for r in group) == list(range(101))
            values.append(median(r['ns'] for r in group if r['sample'] > 0))
        changes = [100 * (values[b] / values[a] - 1) for a, b in [(1, 0), (2, 3), (4, 5), (7, 6)]]
        print(inputs, backend, 'run medians ns:', values, 'paired change %:', changes)
