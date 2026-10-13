"""Independently check C's committed raw receipts, counts and physical accounting.
Usage: python3 tools/verify_c_bench_v2.py
"""
from collections import Counter, defaultdict
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DIR = ROOT / 'bench/results/c-linux-v2'
rows = [json.loads(line) for line in (DIR / 'v2-raw.jsonl').read_text().splitlines()]
runs = [json.loads(line) for line in (DIR / 'v2-runs.jsonl').read_text().splitlines()]
summary = json.loads((DIR / 'v2-summary.json').read_text())

# Source fixture arguments: batch_id=17, price_limit=700; no imported Rust oracle.
def receipt(n, case):
    total = previous = 0
    for i in range(n):
        price, quantity = i * 37 % 1000, i % 9 - 2
        if quantity > 0 and price <= 700 and (case != 'stateful_branch' or price >= previous):
            total += price * quantity
            if case == 'stateful_branch':
                previous = price
    return total + 17

counts = Counter(r['kind'] for r in rows)
assert len(runs) == 234 and all(r['status'] == 'completed' for r in runs)
assert counts['latency'] == 1008 and counts['request'] == 5472
for r in rows:
    if r['kind'] in ['latency', 'request']:
        assert r['status'] == 'completed' and r['receipt'] == receipt(r['rows'], r['case']), r
    if r['kind'] == 'request':
        assert r['latency_ns'] == r['queue_ns'] + r['service_ns']
    if r['kind'] == 'latency':
        assert r['allocation_diagnostics'] is None
    if r['kind'] == 'identity' and 'allocation_diagnostics' in r:
        assert r['allocation_diagnostics'] is False
memory = defaultdict(list)
requests = defaultdict(list)
for r in rows:
    if r['kind'] == 'memory':
        memory[r['command_index']].append(r)
    elif r['kind'] == 'request':
        requests[r['command_index'], r['engine']].append(r)
for r in rows:
    if r['kind'] == 'concurrent':
        group = requests[r['command_index'], r['engine']]
        assert len(group) == r['requests'] == r['completed'] and r['failed'] == 0
        assert len({x['request'] for x in group}) == r['requests']
for group in memory.values():
    peaks = [r['physical']['peak_rss_bytes'] for r in group]
    assert all(v is not None for v in peaks) and peaks == sorted(peaks), peaks
    for r in group:
        if r['engine'] == 'monty-clear' and r['phase'] == 'second_wait':
            assert r['payload_live_logical_bytes'] == r['tasks'] * 16
assert not summary['failures']
result = {'source': json.loads((DIR / 'build-identity.json').read_text())['optimized_source'], 'commands': summary['commands'],
          'raw_rows': len(rows), 'latency_receipts_independently_recomputed': counts['latency'],
          'request_receipts_independently_recomputed': counts['request'],
          'memory_commands_with_present_monotonic_observed_HWM': len(memory),
          'latency_allocation_diagnostics': 'all null', 'speed_identity_allocation_diagnostics': 'all false',
          'no_failures': True, 'independent_repetitions_per_cell': 3,
          'note': 'Finite request observations are not 6480 independent process repetitions.'}
(DIR / 'verification.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
