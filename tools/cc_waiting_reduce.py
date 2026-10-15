#!/usr/bin/env python3
"""Replay controlled backing accounting. No RSS/latency inference."""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EVIDENCE = ROOT / 'evidence/cc-waiting'


def load(name):
    data = json.loads((EVIDENCE / name).read_text())
    assert all(c['exit_code'] == 0 for c in data['commands'])
    rows = [json.loads(line.removeprefix('CC_WAIT '))
            for line in data['commands'][0]['stdout'].splitlines()
            if line.startswith('CC_WAIT ')]
    keyed = {(r['native'], r['mode']): r for r in rows}
    assert len(rows) == len(keyed) == 12
    assert {m for _, m in keyed} == {'dead', 'live', 'tight', 'long', 'small', 'escaped'}
    assert all(r['concurrency'] == 8 for r in rows)
    return keyed


def reduce():
    a, b = load('baseline.json'), load('final.json')
    result = []
    for native in (False, True):
        for mode in ('dead', 'live', 'tight', 'long', 'small', 'escaped'):
            x, y = a[native, mode], b[native, mode]
            assert x['source_payload_bytes'] == y['source_payload_bytes']
            if mode == 'dead':
                assert x['deallocated_source_payload_bytes'] == 0
                assert x['waiting_argument_payload_or_capacity_bytes'] == x['source_payload_bytes']
                assert y['deallocated_source_payload_bytes'] == y['source_payload_bytes']
                assert y['waiting_argument_payload_or_capacity_bytes'] == y['copied_payload_bytes'] == 80
            else:
                assert x['deallocated_source_payload_bytes'] == y['deallocated_source_payload_bytes']
                assert x['waiting_argument_payload_or_capacity_bytes'] == y['waiting_argument_payload_or_capacity_bytes']
                assert x['copied_payload_bytes'] == y['copied_payload_bytes'] == 0
            result.append(dict(backend='native' if native else 'tape', mode=mode,
                               baseline_waiting_argument_payload_or_capacity_bytes=x['waiting_argument_payload_or_capacity_bytes'],
                               final_waiting_argument_payload_or_capacity_bytes=y['waiting_argument_payload_or_capacity_bytes'],
                               final_deallocated_source_payload_bytes=y['deallocated_source_payload_bytes']))
    return dict(cohorts_per_arm=len(a), tasks_per_arm=sum(r['concurrency'] for r in a.values()), rows=result)


if __name__ == '__main__':
    print(json.dumps(reduce(), indent=2))
