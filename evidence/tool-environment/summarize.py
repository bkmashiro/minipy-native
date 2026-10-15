import collections
import gzip
import json
import pathlib
import statistics

path = pathlib.Path(__file__).with_name('requests.jsonl.gz')
with gzip.open(path, 'rt') as stream:
    rows = [json.loads(line) for line in stream]
assert len(rows) == 2624
groups = collections.defaultdict(list)
for row in rows:
    if row['sample'] > 0:
        key = (row['tools'], row['native'], row['used'], row['round'], row['registered'])
        groups[key].append(row['ns'])
assert len(groups) == 64 and all(len(v) == 40 for v in groups.values())
for key in sorted(groups):
    if key[-1]:
        continue
    ordinary = statistics.median(groups[key])
    registered = statistics.median(groups[key[:-1] + (True,)])
    print(json.dumps(dict(tools=key[0], backend='native' if key[1] else 'tape',
                          used=key[2], round=key[3], ordinary_ns=ordinary,
                          registered_ns=registered, delta_pct=100*(registered/ordinary-1))))
