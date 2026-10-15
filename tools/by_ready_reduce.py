#!/usr/bin/env python3
"""Recompute the bounded BY timing and separate allocation/layout evidence."""
import json
from pathlib import Path
import statistics
from perf_workbench import ROOT, reduce


def main():
    base = ROOT / 'evidence/by-ready'
    timing = {name: reduce(base / f'{name}.jsonl.gz') for name in ('ab', 'aa')}
    for name, data in timing.items():
        assert data['complete'] and data['observed_processes'] == 64
        assert data['measured_samples'] == 3072 and not data['failures']
        print(name, data['observed_processes'], 'processes,', data['measured_samples'], 'measured requests')
        for cell in data['cells']:
            a, b = (cell['medians_ns'][arm] for arm in 'AB')
            print(cell['backend'], cell['case'], f'A={a:.2f} B={b:.2f} ns B:A={b/a:.4f}')
    for arm in ('baseline', 'candidate'):
        data = json.loads((base / f'{arm}-diagnostic.json').read_text())
        assert data['commands'][0]['exit_code'] == 0
        rows = [json.loads(line.partition('BY ')[2]) for line in data['commands'][0]['stdout'].splitlines() if 'BY ' in line]
        assert len(rows) == 20
        for native in (False, True):
            for mode in (0, 1):
                samples = [r for r in rows if r['native'] == native and r['mode'] == mode and r['sample'] > 0]
                assert len(samples) == 4
                metrics = {key: statistics.median(r[key] for r in samples)
                           for key in ('future_bytes', 'allocations', 'requested_bytes')}
                print(arm, 'native' if native else 'tape', 'ready' if mode else 'deferred', metrics)


if __name__ == '__main__':
    main()
