"""Recompute E tables and audit the raw profiler invariants (stdlib only)."""
import collections
import gzip
import json
from pathlib import Path
import statistics as stats
import sys

CASES = ['empty', 'scalar', 'scan', 'weighted', 'two_scan', 'legacy']
PHASES = ['frontend_ns', 'setup_ns', 'lowering_ns', 'backend_ns',
          'finalize_ns', 'cleanup_ns']


def median_us(rows, field):
    return stats.median(r[field] for r in rows) / 1000


def summarize(path):
    with gzip.open(path, 'rt') as f:
        rows = [json.loads(line) for line in f]
    assert len(rows) == 2208
    compiles = [r for r in rows if r['kind'] == 'compile']
    inventories = [r for r in rows if r['kind'] == 'inventory']
    assert len(compiles) == 2196 and len(inventories) == 12
    for case in CASES:
        for diag in [False, True]:
            for round_id in range(3):
                samples = [r for r in compiles if (r['case'], r['diagnostic'], r['round'])
                           == (case, diag, round_id)]
                assert [r['sample'] for r in samples] == list(range(61))
        assert len({r['code_bytes'] for r in compiles if r['case'] == case}) == 1
    for r in compiles:
        if not r['diagnostic']:
            assert not any(k in r for k in PHASES)
            continue
        assert r['compile_ns'] >= sum(r[k] for k in PHASES)
        assert r['frontend_ns'] >= r['parse_ns'] + r['type_ns']
        passes = {p['name']: p for p in r['passes']}
        assert passes['verifier']['calls'] == 5
        assert passes['compile']['calls'] == 1
        assert all(p['inclusive_ns'] >= p['exclusive_ns'] >= 0 for p in passes.values())
        # All active timers are inside the root compile pass. Sum self, not total.
        assert sum(p['exclusive_ns'] for p in passes.values()) == passes['compile']['inclusive_ns']
        assert r['backend_ns'] >= passes['compile']['inclusive_ns']
    result = {'raw_rows': len(rows), 'cases': {}}
    for case in CASES:
        item = {}
        sets = {}
        for diag, mode in [(False, 'plain'), (True, 'diagnostic')]:
            all_rows = [r for r in compiles if r['case'] == case and r['diagnostic'] == diag]
            warm = [r for r in all_rows if r['sample'] >= 6]
            sets[mode] = warm
            item[mode] = {
                'n': len(warm), 'compile_us': median_us(warm, 'compile_ns'),
                'drop_us': median_us(warm, 'drop_ns'),
                'compile_plus_drop_us': stats.median(r['compile_ns'] + r['drop_ns'] for r in warm) / 1000,
                'process_first_compile_us': [r['compile_ns'] / 1000 for r in all_rows if r['sample'] == 0],
                'samples_1_to_5_median_us': median_us([r for r in all_rows if 1 <= r['sample'] <= 5], 'compile_ns'),
                'round_compile_us': [median_us([r for r in warm if r['round'] == j], 'compile_ns') for j in range(3)],
            }
        item['overhead_pooled_us'] = item['diagnostic']['compile_us'] - item['plain']['compile_us']
        item['overhead_round_pct'] = [100 * (d / p - 1) for d, p in zip(
            item['diagnostic']['round_compile_us'], item['plain']['round_compile_us'])]
        warm = sets['diagnostic']
        item['phases_us'] = {k: median_us(warm, k) for k in PHASES + ['parse_ns', 'type_ns']}
        item['unattributed_us'] = stats.median(r['compile_ns'] - sum(r[k] for k in PHASES) for r in warm) / 1000
        item['backend_wall_share_pct'] = stats.median(100 * r['backend_ns'] / r['compile_ns'] for r in warm)
        by_pass = collections.defaultdict(list)
        for r in warm:
            for p in r['passes']:
                by_pass[p['name']].append((p, r['backend_ns'], r['compile_ns']))
        item['passes'] = {
            name: {'inclusive_us': stats.median(p['inclusive_ns'] for p, _, _ in group) / 1000,
                   'exclusive_us': stats.median(p['exclusive_ns'] for p, _, _ in group) / 1000,
                   'exclusive_backend_pct': stats.median(100 * p['exclusive_ns'] / backend for p, backend, _ in group),
                   'exclusive_compile_pct': stats.median(100 * p['exclusive_ns'] / wall for p, _, wall in group)}
            for name, group in by_pass.items()
        }
        item['inventory'] = {r['when']: {k: v for k, v in r.items() if k not in ['kind', 'case', 'when', 'round']}
                             for r in inventories if r['case'] == case}
        result['cases'][case] = item
    return result


if __name__ == '__main__':
    root = Path(__file__).resolve().parents[1]
    path = Path(sys.argv[1]) if len(sys.argv) > 1 else root / 'bench/results/e-compiler-profile/linux.jsonl.gz'
    print(json.dumps(summarize(path), indent=2))
