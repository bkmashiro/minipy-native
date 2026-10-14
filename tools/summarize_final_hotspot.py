"""Recompute K matched raw. Stdlib only; H/J process-CPU accounting."""
from collections import defaultdict
import gzip
import json
from pathlib import Path
import statistics as stats
from summarize_average_ab import mix_summary

OUT = Path(__file__).resolve().parents[1] / 'bench/results/k-final-hotspot'


def shape_summary(rows):
    return {f'{metric}_mean_us': stats.mean(r[metric + '_ns'] for r in rows) / 1000
            for metric in ['cpu', 'wall', 'prepare', 'execute']}


def summarize():
    mixed, shapes = defaultdict(list), defaultdict(list)
    count = 0
    overload = set()
    seen = set()
    with gzip.open(OUT / 'next-header.jsonl.gz', 'rt') as f:
        for line in f:
            r = json.loads(line)
            count += 1
            assert r['completed'] and r['engine'] == 'interpreter'
            key = (r['build'], r['round'], r['case'], r['rows'], r['mode'],
                   r.get('mix'), r.get('delay_us', 0), r['sample'])
            # Mixed windows contain multiple identical shapes; standalone must not.
            if r['kind'] != 'mixed':
                assert key not in seen
                seen.add(key)
            if r['kind'] == 'mixed':
                if r['overloaded']:
                    overload.add((r['build'], r['round'], r['mix'], r['mode'], r['delay_us'], r['sample']))
                if r['sample'] >= 2:
                    mixed[f"{r['mix']}/{r['mode']}/{r['delay_us']}/{r['build']}"].append(r)
            elif r['sample'] >= 11:
                shapes[f"{r['case']}:{r['rows']}/{r['mode']}/{r['build']}"].append(r)
    identity = json.loads((OUT / 'next-header-identity.json').read_text())
    assert count == identity['raw_count'] == 20560
    result = {'raw_count': count, 'all_raw_overloaded_batches': len(overload),
              'mixed': {}, 'shapes': {}}
    assert len(mixed) == 12 and len(shapes) == 20
    for key, rows in sorted(mixed.items()):
        assert len(rows) == 4 * 7 * 40
        result['mixed'][key] = mix_summary(rows) | {
            'rounds': [mix_summary([r for r in rows if r['round'] == i]) for i in range(4)]}
    for key, rows in sorted(shapes.items()):
        assert len(rows) == 4 * 30
        result['shapes'][key] = shape_summary(rows) | {
            'rounds': [shape_summary([r for r in rows if r['round'] == i]) for i in range(4)]}
    return result


if __name__ == '__main__':
    result = summarize()
    (OUT / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    print('raw', result['raw_count'], 'overloaded', result['all_raw_overloaded_batches'])
    for category in ['mixed', 'shapes']:
        metric = 'cpu_us_per_task' if category == 'mixed' else 'cpu_mean_us'
        for key, base in result[category].items():
            if not key.endswith('/base'):
                continue
            candidate = result[category][key.removesuffix('/base') + '/candidate']
            print(key.removesuffix('/base'), 'CPU', round(base[metric], 3), '->',
                  round(candidate[metric], 3), 'change%', round(100 * (candidate[metric] / base[metric] - 1), 2),
                  'paired changes%', [round(100 * (c[metric] / b[metric] - 1), 2)
                                      for b, c in zip(base['rounds'], candidate['rounds'])])
            if category == 'mixed':
                print('  mean/p95/p99', [(round(base[m], 2), round(candidate[m], 2))
                                          for m in ['mean_latency_us', 'short_p95_us', 'short_p99_us']])
