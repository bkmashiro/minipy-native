#!/usr/bin/env python3
"""Reduce bounded BW evidence. Formal requests and allocation diagnostics stay separate."""
import json
from pathlib import Path
import re
import perf_workbench

ROOT = Path(__file__).resolve().parents[1] / 'evidence/bw-sink'


def main():
    result = {}
    for name in ('ab', 'aa'):
        data = perf_workbench.reduce(ROOT / (name + '.jsonl.gz'))
        assert data['complete'] and not data['diagnostic']
        result[name] = {
            'processes': data['observed_processes'],
            'measured_requests': data['measured_samples'],
            'startup_median_ns': data['startup_median_ns'],
            'cells': [{k: row[k] for k in ('backend', 'case', 'medians_ns', 'b_over_a')}
                      for row in data['cells']],
        }
    for name in ('baseline', 'candidate'):
        rows = json.loads((ROOT / (name + '-diagnostic.json')).read_text())
        assert [r['exit'] for r in rows] == ([101, 0] if name == 'baseline' else [0, 0])
        counts = re.findall(r'native=(false|true): plain=(\d+), joined=(\d+)', rows[0]['stdout'])
        assert len(counts) == 2
        layout = re.search(r'sink_layout (.+)', rows[1]['stdout'])
        assert layout is not None
        result[name + '_diagnostic'] = {
            'allocations': {native: {'plain': int(plain), 'joined': int(joined)}
                            for native, plain, joined in counts},
            'layout': layout.group(1),
            'allocation_test_expected_failure': name == 'baseline',
        }
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
