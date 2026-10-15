#!/usr/bin/env python3
"""Replay BZ formal requests and independent backing-owner observations."""
import json
from pathlib import Path
from perf_workbench import ROOT, reduce

path = ROOT / 'evidence/bz-detach'
formal = {name: reduce(path / f'{name}.jsonl.gz') for name in ('ab', 'aa')}
for name, result in formal.items():
    assert result['complete'] and not result['diagnostic']
    assert result['aa'] == (name == 'aa')
    assert result['observed_processes'] == 64
    assert result['measured_samples'] == 2048
    assert len(result['cells']) == 8
rows = []
layouts = {}
for arm in ('baseline', 'candidate'):
    record = json.loads((path / f'{arm}-diagnostic.json').read_text())
    assert len(record['commands']) == 1 and record['commands'][0]['exit_code'] == 0
    lines = record['commands'][0]['stdout'].splitlines()
    data = [json.loads(line.partition('BZ ')[2]) for line in lines if line.startswith('BZ ')]
    layout = [json.loads(line.partition('BZ_LAYOUT ')[2]) for line in lines if line.startswith('BZ_LAYOUT ')]
    assert len(data) == len(layout) == 12
    assert {(r['native'], r['mode']) for r in data} == {(n, m) for n in (False, True) for m in ('dead', 'live', 'escaped', 'tight', 'large', 'small')}
    for row in data:
        detached = row['mode'] == 'escaped' or (arm == 'candidate' and row['mode'] == 'dead')
        assert row['backing_released_before_wait'] == detached
        assert row['copied_bytes'] == (10 if arm == 'candidate' and row['mode'] == 'dead' else 0)
        if row['mode'] == 'dead':
            assert row['root_bytes'] == 1_048_621
            assert row['waiting_argument_owner_bytes'] == (10 if arm == 'candidate' else 1_048_621)
        if not detached:
            assert row['waiting_argument_owner_bytes'] == row['root_bytes']
    layouts[arm] = sorted(set(r['wrapper_future_bytes'] for r in layout))
    rows.extend(dict(arm=arm, **r) for r in data)
assert layouts['baseline'] == layouts['candidate']
summary = {'formal': formal, 'owner_diagnostic': rows, 'wrapper_future_bytes': layouts}
print(json.dumps(summary, indent=2))
