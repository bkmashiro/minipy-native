#!/usr/bin/env python3
"""Read-only CL replay: source reconstruction, request census, layout and real code."""
import gzip
import hashlib
import json
from pathlib import Path
import re
import subprocess
from ce_overflow_reduce import apply
from perf_workbench import reduce

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'evidence/cl-dest'
BASE = '552faedd030d2a039b93aa04ca6005a3e991a0b3'
CASES = {'scalar', 'branch', 'madd_short', 'madd_loop', 'ready', 'escaped_join', 'detach_wait'}

def digest(data):
    return hashlib.sha256(data).hexdigest()

def load(path):
    with gzip.open(path, 'rt') as f:
        return json.load(f)

def main():
    patch = (OUT / 'candidate.patch').read_text()
    patches = {p.splitlines()[0].split()[1][2:]: p for p in patch.split('diff --git ')[1:]}
    assert set(patches) == {'src/interpreter/tape.rs', 'src/interpreter/debug.rs'}
    def source(path, candidate):
        data = subprocess.check_output(['git', 'show', f'{BASE}:{path}'], cwd=ROOT).decode()
        return (apply(data, patches[path]) if candidate and path in patches else data).encode()
    manifests = {}
    timing = {}
    for name in ('ab', 'aa'):
        path = OUT / f'{name}.jsonl.gz'
        result = reduce(path)
        assert result['complete'] and result['observed_processes'] == 56 and result['measured_samples'] == 1344
        with gzip.open(path, 'rt') as f:
            m = json.loads(next(f))
        assert m['rounds'] == 1 and m['reps'] == 24 and not m['diagnostic']
        assert m['aa'] == (name == 'aa') and m['order'] == 'ABBA'
        assert m['entries'] == {'A': 'registered', 'B': 'registered'}
        assert m['driver_sha256'] == digest((ROOT / 'tools/perf_workbench.py').read_bytes())
        assert {tuple(c) for c in m['cells']} == {(b, c) for b in ('tape', 'native') for c in CASES}
        for arm, identity in m['arms'].items():
            assert identity['commit'] == BASE
            for p, h in identity['sources'].items():
                assert digest(source(p, name == 'ab' and arm == 'B')) == h, (name, arm, p)
        if name == 'aa':
            assert m['arms']['A'] == m['arms']['B']
        manifests[name] = m
        timing[name] = result
    assert manifests['ab']['arms']['A'] == manifests['aa']['arms']['A']
    diag = load(OUT / 'diagnostic.json.gz')
    assert diag['inputs'] == {
        'candidate_sha256': digest(patch.encode()),
        'overlay_sha256': digest((ROOT / 'evidence/ci-layout/diagnostic.patch').read_bytes()),
        'probe_sha256': digest((ROOT / 'evidence/ci-layout/probe.rs').read_bytes()),
    }
    layouts = {}
    cells = {}
    machines = {}
    native_code = {}
    for arm in 'AB':
        data = diag[arm]
        assert data['formal_binary_sha256'] == manifests['ab']['arms'][arm]['binary_sha256']
        assert all(r['exit_code'] == 0 for r in data['records'])
        for p, h in data['sources'].items():
            assert digest(source(p, arm == 'B')) == h, (arm, p)
        text = '\n'.join(r['stdout'] for r in data['records'])
        layout = json.loads(text.split('CI_LAYOUT ', 1)[1].splitlines()[0])
        assert layout['op'] == (56 if arm == 'A' else 48)
        assert layout['dest'] == (16 if arm == 'A' else 4)
        assert all(layout[k] == v for k, v in {'op_align': 8, 'operand': 16, 'unit': 112, 'state': 312, 'scratch': 4096, 'native_frame': 320}.items())
        layouts[arm] = layout
        rows = [json.loads(s) for s in re.findall(r'CI_CASE (.*)', text)]
        assert len(rows) == 9
        cells[arm] = {r['name']: r for r in rows}
        futures = re.findall(r'CI_FUTURE (\w+) (\w+) (\d+)', text)
        assert len(futures) == 18 and all(int(n) == (928 if b == 'tape' else 664) for _, b, n in futures)
        assert len(re.findall(r'CI_EXECUTED .* both status=Ok stdout=exact', text)) == 9
        native_code[arm] = re.findall(r'CI_NATIVE_SIZE (\w+) (\d+)', text)
        machine = next(r['stdout'] for r in data['records'] if any('--disassemble-symbols=' in c for c in r['command']))
        n = len(re.findall(r'^\s*[0-9a-f]+:', machine, re.M))
        assert n == data['instructions'] == (1907 if arm == 'A' else 1875)
        assert ('#0x38' if arm == 'A' else '#0x30') in machine
        assert '#0x1000' in machine and '#0x200' in machine
        if arm == 'B':
            assert re.search(r'ldrh\s+w\d+, \[x19, #0x4\]', machine)
            assert re.search(r'ldrb\s+w\d+, \[x19, #0x3\]', machine)
            assert not re.search(r'\buxt[bhw]\b', machine)
        machines[arm] = dict(instructions=n, bytes=n * 4, sync_stack_reserved=4256)
    assert native_code['A'] == native_code['B']
    saving = []
    for name, a in cells['A'].items():
        b = cells['B'][name]
        assert all(a[k] == b[k] for k in a if k != 'prepared_bytes')
        saved = a['capacity'] * 8
        assert a['prepared_bytes'] - b['prepared_bytes'] == saved
        saving.append(dict(case=name, ops=a['ops'], capacity=a['capacity'], used_bytes_A=a['ops'] * 56, used_bytes_B=b['ops'] * 48,
                           capacity_bytes_A=a['capacity'] * 56, capacity_bytes_B=b['capacity'] * 48, saved_capacity_bytes=saved,
                           prepared_bytes_A=a['prepared_bytes'], prepared_bytes_B=b['prepared_bytes']))
    verification = load(OUT / 'verification.json.gz')
    assert len(verification['records']) == 11 and all(r['exit_code'] == 0 for r in verification['records'])
    for p, h in verification['sources'].items():
        assert digest(source(p, True)) == h
        assert digest((ROOT / p).read_bytes()) == h
    for key, path in [('test_sha256', ROOT / 'tests/cl_dest.rs'), ('bounds_sha256', OUT / 'bounds.rs'), ('bounds_patch_sha256', OUT / 'bounds.patch')]:
        assert digest(path.read_bytes()) == verification[key]
    bound = '\n'.join(r['stdout'] for r in verification['records'])
    assert 'CL_BOUNDS locals=32 temps=512 highest_temp=511' in bound
    assert 'test high_temporary_debug_step_preserves_index ... ok' in bound
    assert 'test debug_reports_unchanged_local_destination ... ok' in bound
    summary = dict(base=BASE, decision='retain minimal representation for deterministic backing density; no general latency claim; stop CI-CL phase',
                   timing=timing, layouts=layouts, backing=saving, machine=machines, future_bytes={'tape': 928, 'native': 664},
                   formal_processes=112, formal_measured_requests=2688, pmu_measured=False, rss_measured=False,
                   frontier={'admitted_AST_nodes': 512, 'admitted_temp_slots': 477, 'highest_admitted_temp': 476, 'synthetic_sealed_round_trip_slots': 512})
    (OUT / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    aa = {(c['backend'], c['case']): c for c in timing['aa']['cells']}
    for c in timing['ab']['cells']:
        print(c['backend'], c['case'], c['medians_ns'], round(c['b_over_a'], 5), round(aa[c['backend'], c['case']]['b_over_a'], 5))
    print('machine', machines, 'backing saved', [(r['case'], r['saved_capacity_bytes']) for r in saving])

if __name__ == '__main__':
    main()
