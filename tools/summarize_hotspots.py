"""Recompute J raw with H's same batch/nearest-rank accounting."""
from collections import defaultdict
import gzip
import json
from pathlib import Path
import statistics as stats
from summarize_average_ab import mix_summary
OUT = Path(__file__).resolve().parents[1] / 'bench/results/j-hotspots'
def summarize(path):
    mixed, shapes = defaultdict(list), defaultdict(list)
    count = 0
    overload = set()
    with gzip.open(path, 'rt') as f:
        for line in f:
            r = json.loads(line)
            count += 1
            if r['kind'] == 'mixed':
                if r['overloaded']:
                    overload.add((r['build'], r['engine'], r['mix'], r['mode'], r['delay_us'], r['round'], r['chunk'], r['sample']))
                if r['sample'] >= 2:
                    mixed[f"{r['chunk']}/{r['mix']}/{r['mode']}/{r['delay_us']}/{r['build']}/{r['engine']}"].append(r)
            elif r['sample'] >= 11:
                shapes[f"{r['case']}:{r['rows']}/{r['mode']}/{r['build']}"].append(r)
    identity = json.loads((OUT / (path.name.replace('.jsonl.gz', '') + '-identity.json')).read_text())
    assert count == identity['raw_count']
    result = {'raw_count': count, 'overloaded_batches': len(overload), 'mixed': {}, 'shapes': {}}
    for k, rs in sorted(mixed.items()):
        rounds = sorted({r['round'] for r in rs})
        result['mixed'][k] = mix_summary(rs) | {'rounds': [mix_summary([r for r in rs if r['round'] == i]) for i in rounds]}
    for k, rs in sorted(shapes.items()):
        assert len(rs) == 30
        result['shapes'][k] = {f'{metric}_mean_us': stats.mean(r[metric + '_ns'] for r in rs) / 1000 for metric in ['cpu', 'wall', 'prepare', 'execute']}
    return result
if __name__ == '__main__':
    result = {p.name.replace('.jsonl.gz', ''): summarize(p) for p in sorted(OUT.glob('*.jsonl.gz'))}
    (OUT / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    for label, s in result.items():
        print(label, s['raw_count'], 'overloaded', s['overloaded_batches'])
        for k, v in s['mixed'].items():
            print(k, 'CPU', round(v['cpu_us_per_task'], 2), 'mean', round(v['mean_latency_us'], 2), 'p95', round(v['short_p95_us'], 2), 'p99', round(v['short_p99_us'], 2), 'roundCPU', [round(r['cpu_us_per_task'], 2) for r in v['rounds']], 'roundP99', [round(r['short_p99_us'], 2) for r in v['rounds']])
        for k, v in s['shapes'].items():
            print(k, 'CPU', round(v['cpu_mean_us'], 2), 'prep', round(v['prepare_mean_us'], 2))
