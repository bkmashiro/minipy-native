"""Recompute G summaries from committed raw, no measurements."""
import gzip
import json
from collections import defaultdict
from pathlib import Path
from statistics import median
ROOT = Path(__file__).resolve().parents[1] / 'bench/results/g-interpreter'
rows = [json.loads(l) for l in gzip.open(ROOT / 'linux.jsonl.gz', 'rt')]
assert len(rows) == 18090
by = defaultdict(list)
for r in rows:
    assert r['completed']
    if r['sample'] >= 101:
        by[(r['engine'], r['case'], r['rows'], r['mode'])].append(r)
speed = []
for (engine, case, n, mode), rs in sorted(by.items()):
    assert len(rs) == 300
    speed.append({'engine': engine, 'case': case, 'rows': n, 'mode': mode, 'samples': len(rs), **{k + '_us': median(r[k] for r in rs) / 1000 for k in ['wall_ns', 'cpu_ns', 'prepare_ns', 'execute_ns']}, 'round_wall_us': [median(r['wall_ns'] for r in rs if r['round'] == i) / 1000 for i in range(3)], 'prepared_bytes': rs[0]['prepared_bytes']})
mem = [json.loads(l) for l in (ROOT / 'memory.jsonl').read_text().splitlines()]
assert len(mem) == 24
memory = []
for engine in ['native', 'interpreter', 'monty']:
    for n in [100, 10000]:
        rs = [r for r in mem if r['engine'] == engine and r['rows'] == n]
        start = rs[0]
        for r in rs[1:]:
            increment = r['allocation']['live_requested_bytes'] - start['allocation']['live_requested_bytes']
            memory.append({'engine': engine, 'rows': n, 'count': r['count'], 'phase': r['phase'], 'live_increment_requested_bytes': increment, 'per_task_increment_bytes': increment / r['count'], 'input_payload_bytes': r['input_payload_bytes'], 'non_payload_per_task_bytes': (increment - r['input_payload_bytes']) / r['count'], 'rss_increment_bytes': r['physical']['rss_bytes'] - start['physical']['rss_bytes'], 'pss_increment_bytes': r['physical']['pss_bytes'] - start['physical']['pss_bytes']})
clear_rows = [json.loads(l) for l in gzip.open(ROOT / 'clear-linux.jsonl.gz', 'rt')]
assert len(clear_rows) == 4824
clear_speed = []
clear_groups = defaultdict(list)
for r in clear_rows:
    assert r['completed']
    if r['sample'] >= 101:
        clear_groups[(r['engine'], r['case'], r['mode'])].append(r)
for (engine, case, mode), rs in sorted(clear_groups.items()):
    assert len(rs) == 300
    clear_speed.append({'engine': engine, 'case': case, 'mode': mode, 'rows': 10000, 'wall_us': median(r['wall_ns'] for r in rs) / 1000, 'round_wall_us': [median(r['wall_ns'] for r in rs if r['round'] == i) / 1000 for i in range(3)]})
clear_mem = [json.loads(l) for l in (ROOT / 'clear-memory.jsonl').read_text().splitlines()]
assert len(clear_mem) == 16
clear_memory = []
for engine in ['monty', 'monty-clear']:
    for n in [100, 10000]:
        rs = [r for r in clear_mem if r['engine'] == engine and r['rows'] == n]
        base = rs[0]['allocation']['live_requested_bytes']
        record = rs[2]
        clear_memory.append({'engine': engine, 'rows': n, 'count': record['count'], 'record_wait_increment_requested_bytes': record['allocation']['live_requested_bytes'] - base, 'per_task_increment_bytes': (record['allocation']['live_requested_bytes'] - base) / record['count']})
print(json.dumps({'speed': speed, 'memory': memory, 'clear_speed': clear_speed, 'clear_memory': clear_memory}, indent=2))
