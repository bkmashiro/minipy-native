#!/usr/bin/env python3
"""Replay CD request evidence and exact test-only analysis visit counts."""
import gzip
import json
from pathlib import Path
import re
import perf_workbench as pw

ROOT = Path(__file__).resolve().parents[1]
EVIDENCE = ROOT / 'evidence/cd-acyclic'


def diagnostics(name):
    rows = {}
    for line in (EVIDENCE / name).read_text().splitlines():
        match = re.search(r'CD case=(\w+) ops=(\d+) buffer_visits=(\d+) old_buffer_visits=(\d+) json_visits=(\d+) old_json_visits=(\d+)', line)
        if match:
            case, *values = match.groups()
            assert case not in rows
            rows[case] = list(map(int, values))
    assert len(rows) == 10
    return rows


def main():
    baseline = diagnostics('baseline-diagnostic.txt')
    candidate = diagnostics('candidate-diagnostic.txt')
    for case, (ops, b, ob, j, oj) in candidate.items():
        assert baseline[case] == [ops, ob, ob, oj, oj]
        if case.startswith('loop_'):
            assert (b, j) == (ob, oj)
        else:
            assert b == ops and j == (ops if oj else 0)
        print(case, f'buffer {ob}->{b}', f'JSON {oj}->{j}', f'ops={ops}')
    summaries = {label: pw.reduce(EVIDENCE / f'{label}.jsonl.gz') for label in ('ab', 'aa')}
    for label, summary in summaries.items():
        assert summary['complete'] and not summary['diagnostic']
        assert summary['observed_processes'] == 80 and summary['measured_samples'] == 3840
        with gzip.open(EVIDENCE / f'{label}.jsonl.gz', 'rt') as f:
            manifest = json.loads(next(f))
        arms = manifest['arms']
        changed = {p for p in arms['A']['sources'] if arms['A']['sources'][p] != arms['B']['sources'][p]}
        assert changed == (set() if label == 'aa' else {'src/interpreter.rs', 'src/interpreter/tape.rs'})
        assert manifest['aa'] == (label == 'aa')
    aa = {(c['backend'], c['case']): c['b_over_a'] for c in summaries['aa']['cells']}
    for cell in summaries['ab']['cells']:
        print(cell['backend'], cell['case'], 'A/B', round(cell['b_over_a'], 5),
              'A/A', round(aa[cell['backend'], cell['case']], 5), 'ns', cell['medians_ns'])


if __name__ == '__main__':
    main()
