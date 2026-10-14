"""Recompute H summary from small committed raw. No third-party packages."""
from collections import defaultdict
import gzip
import json
import math
from pathlib import Path
import statistics as stats

OUT = Path(__file__).resolve().parents[1] / 'bench/results/h-average'

def q(values, p):
    return sorted(values)[math.ceil(len(values) * p) - 1] / 1000

def mix_summary(rows):
    batches = {}
    for r in rows:
        key = (r['round'], r['sample'])
        batches.setdefault(key, r)
    assert len(rows) == len(batches) * 40
    short = [r['latency_ns'] for r in rows if r['rows'] <= 100]
    scalar = [r['latency_ns'] for r in rows if r['case'] == 'scalar']
    return {
        'tasks': len(rows), 'batches': len(batches),
        'cpu_us_per_task': sum(r['batch_cpu_ns'] for r in batches.values()) / len(rows) / 1000,
        'mean_latency_us': stats.mean(r['latency_ns'] for r in rows) / 1000,
        'short_p95_us': q(short, .95), 'short_p99_us': q(short, .99),
        'scalar_p95_us': q(scalar, .95), 'scalar_p99_us': q(scalar, .99),
        'mean_batch_wall_us': stats.mean(r['batch_wall_ns'] for r in batches.values()) / 1000,
        'overloaded_batches': sum(r['overloaded'] for r in batches.values()),
        'per_shape': {f'{case}:{n}': {'mean_latency_us': stats.mean(r['latency_ns'] for r in rows if (r['case'], r['rows']) == (case, n)) / 1000,
                                      'mean_prepare_us': stats.mean(r['prepare_ns'] for r in rows if (r['case'], r['rows']) == (case, n)) / 1000}
                      for case, n in sorted({(r['case'], r['rows']) for r in rows})},
    }

def main():
    mixed, shapes = defaultdict(list), defaultdict(list)
    count = 0
    all_overload = 0
    with gzip.open(OUT / 'linux.jsonl.gz', 'rt') as f:
        for line in f:
            r = json.loads(line)
            count += 1
            if r['kind'] == 'mixed':
                all_overload += r['overloaded']
                if r['sample'] >= 3:
                    key = f"{r['mix']}/{r['mode']}/{r['delay_us']}/{r['build']}/{r['engine']}"
                    mixed[key].append(r)
            elif r['sample'] >= 21:
                key = f"{r['case']}:{r['rows']}/{r['mode']}/{r['build']}/{r['engine']}"
                shapes[key].append(r)
    assert count == 72600
    result = {'raw_count': count, 'all_raw_overloaded_batches': all_overload // 40,
              'mixed': {}, 'shapes': {}, 'memory': {}}
    for key, rows in sorted(mixed.items()):
        result['mixed'][key] = mix_summary(rows)
        result['mixed'][key]['rounds'] = [mix_summary([r for r in rows if r['round'] == i]) for i in range(3)]
    for key, rows in sorted(shapes.items()):
        assert len(rows) == 240
        def summarize(rs):
            return {f'{metric}_mean_us': stats.mean(r[metric + '_ns'] for r in rs) / 1000 for metric in ['cpu', 'wall', 'prepare', 'execute']} | {'wall_median_us': stats.median(r['wall_ns'] for r in rs) / 1000}
        result['shapes'][key] = summarize(rows)
        result['shapes'][key]['rounds'] = [summarize([r for r in rows if r['round'] == i]) for i in range(3)]
    memory = defaultdict(list)
    for line in (OUT / 'memory.jsonl').read_text().splitlines():
        r = json.loads(line)
        memory[f"{r['mix']}/{r['build']}/{r['engine']}"].append(r)
    assert sum(map(len, memory.values())) == 40
    for key, rows in memory.items():
        before = rows[0]['allocation']['live_requested_bytes']
        result['memory'][key] = {r['phase']: {
            'live_increment_bytes': r['allocation']['live_requested_bytes'] - before,
            'payload_bytes': r['input_payload_bytes'],
            'overhead_per_arrived_task_bytes': (r['allocation']['live_requested_bytes'] - before - r['input_payload_bytes']) / 40,
            'rss_bytes': r['physical']['rss_bytes'], 'pss_bytes': r['physical']['pss_bytes'],
            'sampled_peak_bytes': r['physical']['peak_rss_bytes'],
        } for r in rows}
    (OUT / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    for k, v in result['mixed'].items():
        print(k, 'CPU/task', round(v['cpu_us_per_task'], 3), 'latency', round(v['mean_latency_us'], 3), 'short p99', round(v['short_p99_us'], 3), 'roundCPU', [round(r['cpu_us_per_task'], 3) for r in v['rounds']])
    for k,v in result['shapes'].items():
        print(k, 'CPU', round(v['cpu_mean_us'], 3), 'wall', round(v['wall_mean_us'], 3))

if __name__ == '__main__':
    main()
