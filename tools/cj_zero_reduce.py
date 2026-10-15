#!/usr/bin/env python3
"""Replay CJ fixed-source timing and separate allocation/layout evidence."""
import gzip
import hashlib
import json
from pathlib import Path
import re
import subprocess
from ce_overflow_reduce import apply
from perf_workbench import reduce

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'evidence/cj-zero-buffer'
BASE = 'e1ee1af7cb7e016504d4dac56e1e05199799b7c2'
NAMES = {'scalar', 'bound_tree', 'madd_short', 'quote', 'madd_loop', 'ready',
         'branch', 'joined_sink', 'async_string'}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def patches(path):
    return {p.splitlines()[0].split()[1][2:]: p
            for p in path.read_text().split('diff --git ')[1:]}


def blob(path):
    return subprocess.check_output(['git', 'show', f'{BASE}:{path}'], cwd=ROOT)


def main():
    patch = patches(OUT/'candidate.patch')
    assert set(patch) == {'src/interpreter.rs', 'src/interpreter/debug.rs',
                          'src/interpreter/liveness_tests.rs'}
    def source(path, candidate):
        data = blob(path)
        return apply(data.decode(), patch[path]).encode() if candidate and path in patch else data
    timing = {}
    manifests = {}
    for name in ['ab', 'aa']:
        path = OUT/f'{name}.jsonl.gz'
        result = reduce(path)
        assert result['complete'] and not result['diagnostic']
        assert result['observed_processes'] == 112 and result['measured_samples'] == 2688
        with gzip.open(path, 'rt') as f:
            m = json.loads(next(f))
        assert m['rounds'] == 2 and m['reps'] == 24
        assert m['entries'] == {'A': 'registered', 'B': 'registered'}
        assert m['driver_sha256'] == digest((ROOT/'tools/perf_workbench.py').read_bytes())
        if name == 'aa':
            assert m['arms']['A'] == m['arms']['B']
        for arm, identity in m['arms'].items():
            assert identity['commit'] == BASE
            for p, expected in identity['sources'].items():
                assert digest(source(p, arm == 'B' and name == 'ab')) == expected, (name, arm, p)
        timing[name], manifests[name] = result, m
    assert manifests['aa']['arms']['A'] == manifests['ab']['arms']['A']
    with gzip.open(OUT/'diagnostic.json.gz', 'rt') as f:
        raw = json.load(f)
    assert raw['base'] == BASE and set(raw['arms']) == {'A', 'B'}
    for p, expected in raw['inputs'].items():
        assert digest((ROOT/p).read_bytes()) == expected, p
    assert all(r['exit_code'] == 0 for r in raw['attempts'])
    diag = {}
    for arm, identity in raw['arms'].items():
        for p, expected in identity['sources'].items():
            assert digest(source(p, arm == 'B')) == expected, (arm, p)
            assert expected == manifests['ab']['arms'][arm]['sources'][p]
        records = identity['records']
        assert len(records) == 6 and all(r['exit_code'] == 0 for r in records)
        layout_text, native_text, alloc_text = [r['stdout'] for r in records[1:4]]
        layout = json.loads(layout_text.split('CI_LAYOUT ', 1)[1].splitlines()[0])
        cases = {c['name']: c for c in [json.loads(s.split('CI_CASE ', 1)[1])
                 for s in layout_text.splitlines() if 'CI_CASE ' in s]}
        executed = re.findall(r'CI_EXECUTED (\w+) both status=Ok stdout=exact', layout_text)
        assert len(executed) == 9 and set(executed) == NAMES == set(cases)
        code = {}
        for part in native_text.split('CI_CODE ')[1:]:
            match = re.search(r'CI_BYTES (\d+)', part)
            assert match is not None
            code[part.splitlines()[0]] = int(match[1])
        assert set(code) == NAMES
        allocations = [json.loads(s.split('CJ_ALLOC ', 1)[1]) for s in alloc_text.splitlines()
                       if 'CJ_ALLOC ' in s]
        assert len(allocations) == 54
        assert {(a['case'], a['native'], a['sample']) for a in allocations} == {
            (name, native, n) for name in NAMES for native in [False, True] for n in range(3)}
        for name, c in cases.items():
            c['native_code_bytes'] = code[name]
            c['future_bytes'] = {}
            for backend in ['tape', 'native']:
                match = re.search(rf'CI_FUTURE {name} {backend} (\d+)', layout_text)
                assert match is not None
                c['future_bytes'][backend] = int(match[1])
        assert identity['dispatcher_instructions'] == len(re.findall(
            r'^\s*[0-9a-f]+:', records[-1]['stdout'], re.M))
        diag[arm] = dict(layout=layout, cases=cases, allocations=allocations,
                         dispatcher_instructions=identity['dispatcher_instructions'])
    assert diag['A']['layout'] == diag['B']['layout']
    changes = []
    for name in sorted(NAMES):
        a, b = [diag[arm]['cases'][name] for arm in 'AB']
        removed = a['live_capacity']*8 if a['buffers'] == 0 else 0
        for field in a:
            if field in ('prepared_bytes', 'live_capacity', 'live_len'):
                continue
            assert a[field] == b[field], (name, field)
        assert a['prepared_bytes'] - b['prepared_bytes'] == removed
        assert b['live_capacity'] == (0 if a['buffers'] == 0 else a['live_capacity'])
        for native in [False, True]:
            groups = [[r for r in diag[arm]['allocations'] if r['case'] == name and r['native'] == native]
                      for arm in 'AB']
            for group in groups:
                assert all({k: v for k, v in r.items() if k != 'sample'} ==
                           {k: v for k, v in group[0].items() if k != 'sample'} for r in group)
            x, y = [g[0] for g in groups]
            expected = removed if not native else 0
            assert x['allocations'] - y['allocations'] == int(expected > 0)
            assert x['requested_bytes'] - y['requested_bytes'] == expected
            changes.append(dict(case=name, native=native, allocations_A=x['allocations'],
                                allocations_B=y['allocations'], removed_requested_bytes=expected))
    result = dict(base=BASE, timing=timing, diagnosis=diag, allocation_changes=changes,
                  hardware_misses_measured=False, rss_measured=False)
    (OUT/'summary.json').write_text(json.dumps(result, indent=2)+'\n')
    aa = {(c['backend'], c['case']): c for c in timing['aa']['cells']}
    for c in timing['ab']['cells']:
        print(c['backend'], c['case'], c['medians_ns'],
              round(c['b_over_a'], 5), round(aa[c['backend'], c['case']]['b_over_a'], 5))
    for row in changes:
        if not row['native']:
            print(row)


if __name__ == '__main__':
    main()
