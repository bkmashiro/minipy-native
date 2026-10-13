"""Recompute D's small raw evidence. No synthetic intervals or merged platforms."""
from collections import defaultdict
import json
import math
from pathlib import Path
from statistics import median

BASE = Path(__file__).resolve().parents[1] / 'bench/results/d-script'
EXPECT = {'scratch-linux': 726, 'scratch-m4': 726, 'spill-linux': 726, 'spill-m4': 726,
          'print-growth-linux': 720, 'owner-isolated-linux': 248, 'opt-none-linux': 960,
          'owner-accounting-linux': 840, 'integrated-linux': 1920, 'legacy-c-vs-d-linux': 972}
summary = {}
for name, count in EXPECT.items():
    rows = [json.loads(x) for x in (BASE / (name + '.jsonl')).read_text().splitlines()]
    assert len(rows) == count, (name, len(rows), count)
    groups = defaultdict(list)
    first = defaultdict(list)
    memory = []
    for row in rows:
        if row.get('kind') in ['script_memory', 'weak_observer_control']:
            memory.append(row)
            continue
        if row.get('kind') == 'identity':
            continue
        if row.get('mode') == 'mixed':
            assert row['completed'] == row['reps']
            assert len(row['observations']) == row['reps']
        else:
            assert row.get('completed', row.get('status') == 'completed'), row
        case = row.get('case', 'weighted')
        mode = row.get('mode', row.get('lifecycle'))
        n = row.get('rows', row.get('rows_long'))
        key = '|'.join(map(str, [case, n, mode, row.get('engine'), row.get('arm', 'final')]))
        sample = row.get('rep', row.get('sample', 0))
        # All raw observations survive. These are warm-process summaries, with
        # one-time compiler/library startup reported separately below.
        discard = 5 if name in ['owner-isolated-linux', 'opt-none-linux', 'owner-accounting-linux'] else 10
        if mode == 'mixed' or sample >= discard:
            groups[key].append(row)
        if sample == 0:
            first[key].append(row)
    result = {'raw_count': len(rows), 'groups': {}, 'first_request': {}, 'memory': memory}
    for key, values in groups.items():
        out = {'observations': len(values), 'wall_median_ns': median(r['total_wall_ns'] for r in values),
               'cpu_median_ns': median(r['total_cpu_ns'] for r in values),
               'round_wall_medians_ns': {str(i): median(r['total_wall_ns'] for r in values if r['round'] == i)
                                         for i in sorted({r['round'] for r in values})}}
        if values[0].get('mode') == 'mixed':
            short = sorted(t for r in values for is_short, t in r['observations'] if is_short)
            out.update(short_count=len(short), short_p99_ns=short[math.ceil(.99 * len(short)) - 1])
        else:
            out['compile_median_ns'] = median(r.get('compile_ns', r.get('phases', {}).get('compile_or_clone_ns', 0)) for r in values)
            out['code_bytes'] = sorted({r['code_bytes'] for r in values if r.get('code_bytes') is not None})
        result['groups'][key] = out
    for key, values in first.items():
        result['first_request'][key] = {'n': len(values), 'wall_median_ns': median(r['total_wall_ns'] for r in values),
                                       'wall_min_ns': min(r['total_wall_ns'] for r in values), 'wall_max_ns': max(r['total_wall_ns'] for r in values)}
    summary[name] = result
(BASE / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('verified', sum(EXPECT.values()), 'raw rows across', len(EXPECT), 'datasets; summary.json written')
