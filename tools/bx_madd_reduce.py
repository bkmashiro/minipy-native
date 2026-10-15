#!/usr/bin/env python3
"""Reduce BX formal requests and separately report layout/preparation diagnostics."""
import json
from pathlib import Path
import re
import perf_workbench

ROOT = Path(__file__).resolve().parents[1] / 'evidence/bx-madd'


def main():
    result = {}
    for name in ('ab', 'aa'):
        data = perf_workbench.reduce(ROOT / (name + '.jsonl.gz'))
        assert data['complete'] and not data['diagnostic']
        assert data['observed_processes'] == 80 and data['measured_samples'] == 3840
        result[name] = {
            'processes': data['observed_processes'],
            'measured_requests': data['measured_samples'],
            'startup_median_ns': data['startup_median_ns'],
            'cells': [{k: row[k] for k in ('backend', 'case', 'medians_ns', 'b_over_a')}
                      for row in data['cells']],
        }
    for name in ('baseline', 'candidate'):
        raw = json.loads((ROOT / (name + '-diagnostic.json')).read_text())
        layout = re.findall(r'BX layout (\w+) (.+)', raw['stdout'])
        assert len(layout) == 3
        result[name + '_diagnostic'] = {
            case: {key: int(value) for key, value in re.findall(r'(\w+)=(\d+)', text)}
            for case, text in layout
        }
        batches = re.findall(r'BX prepare (\w+) batch=(\d+) count=1000 ns=(\d+)', raw['stdout'])
        assert len(batches) == 27
        # Keep all batches, including the initial cold one. These standalone runs
        # are diagnostic only and visibly warm/drift; no causal timing claim.
        result[name + '_preparation_ns_per_request'] = {
            case: [int(ns) / 1000 for c, _, ns in batches if c == case]
            for case, _ in layout
        }
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
