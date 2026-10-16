#!/usr/bin/env python3
"""Read-only CO evidence replay and source/binary identity checks. No timing."""
import gzip
import hashlib
import json
from pathlib import Path
import re
import subprocess
from ce_overflow_reduce import apply
from perf_workbench import reduce

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'evidence/co-count'
BASE = '44212e00e9161b479b6a367409087e82d3520190'
CASES = {'count_price_small', 'count_price_large', 'count_quantity_small', 'count_quantity_large', 'count_none', 'count_all', 'scalar', 'quote', 'ready'}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def load(path):
    with gzip.open(path, 'rt') as f:
        return json.load(f)


def main():
    def patches(name):
        return {p.splitlines()[0].split()[1][2:]: p for p in (OUT / name).read_text().split('diff --git ')[1:]}
    candidate = patches('candidate.patch')
    adapter = patches('adapter.patch')
    assert set(candidate) == {'src/interpreter/tape.rs', 'src/interpreter.rs', 'src/interpreter/liveness_tests.rs'}
    assert set(adapter) == {'tests/perf_workbench.rs', 'tools/perf_workbench.py'}
    def source(path, optimized):
        s = subprocess.check_output(['git', 'show', f'{BASE}:{path}'], cwd=ROOT).decode()
        if path in adapter:
            s = apply(s, adapter[path])
        if optimized and path in candidate:
            s = apply(s, candidate[path])
        return s.encode()
    timing, manifests = {}, {}
    for name in ('ab', 'aa', 'monitored-ab', 'monitored-aa'):
        p = OUT / f'{name}.jsonl.gz'
        r = reduce(p)
        assert r['complete'] and r['observed_processes'] == 72 and r['measured_samples'] == 864
        with gzip.open(p, 'rt') as f:
            m = json.loads(next(f))
        assert m['rounds'] == 1 and m['reps'] == 12 and not m['diagnostic']
        is_aa = name.endswith('aa')
        assert m['aa'] == is_aa and m['order'] == 'ABBA'
        assert m['entries'] == {'A': 'registered', 'B': 'registered'}
        assert m['driver_sha256'] == digest(source('tools/perf_workbench.py', False)) == digest((ROOT / 'tools/perf_workbench.py').read_bytes())
        assert {tuple(c) for c in m['cells']} == {(b, c) for b in ('tape', 'native') for c in CASES}
        for arm, identity in m['arms'].items():
            assert identity['commit'] == BASE and not identity['diagnostic']
            assert identity['command'] == ['cargo', '+1.96.0', 'test', '--offline', '--locked', '--release', '--features', 'tool-json', '--test', 'perf_workbench', '--no-run', '--message-format=json']
            for path, h in identity['sources'].items():
                assert digest(source(path, not is_aa and arm == 'B')) == h, (name, arm, path)
        if is_aa:
            assert m['arms']['A'] == m['arms']['B']
        timing[name], manifests[name] = r, m
    assert manifests['ab']['arms']['A'] == manifests['aa']['arms']['A']
    assert manifests['ab']['arms'] == manifests['monitored-ab']['arms']
    assert manifests['aa']['arms'] == manifests['monitored-aa']['arms']
    cpu = load(OUT / 'cpu.json.gz')
    assert cpu['ready'] and not cpu['rejected'] and cpu['pre_exit'] == 0
    assert len(cpu['records']) == 2 and all(r['exit_code'] == 0 for r in cpu['records'])
    assert cpu['binaries'] == {a: manifests['ab']['arms'][a]['binary_sha256'] for a in 'AB'}
    pre_idle = list(map(float, re.findall(r'([\d.]+)% idle', cpu['pre'])))
    during_idle = list(map(float, re.findall(r'([\d.]+)% idle', cpu['monitor_text'])))
    assert len(pre_idle) == 3 and min(pre_idle[-2:]) >= 90
    assert len(during_idle) == 2 and during_idle[-1] >= 85
    diag = load(OUT / 'diagnostic.json.gz')
    for key, path in [('candidate_sha256', 'candidate.patch'), ('overlay_sha256', 'diagnostic.patch'), ('probe_sha256', 'probe.rs')]:
        assert diag['inputs'][key] == digest((OUT / path).read_bytes())
    assert not diag['inputs']['environment']['RUSTFLAGS'] and not diag['inputs']['environment']['CARGO_ENCODED_RUSTFLAGS']
    layouts, descriptors, counts = {}, {}, {}
    for arm in 'AB':
        d = diag[arm]
        assert d['formal_binary_sha256'] == manifests['ab']['arms'][arm]['binary_sha256']
        assert all(r['exit_code'] == 0 for r in d['records'])
        for path, h in d['sources'].items():
            assert digest(source(path, arm == 'B')) == h
        text = '\n'.join(r['stdout'] for r in d['records'])
        layout = dict((k, int(v)) for k, v in re.findall(r'(\w+)=(\d+)', text.split('CO_LAYOUT ', 1)[1].splitlines()[0]))
        assert layout == dict(op=48, operand=16, unit=112, state=312, scratch=4096)
        layouts[arm] = layout
        futures = re.findall(r'CO_FUTURE (\w+) (tape|native)=(\d+)', text)
        assert len(futures) == 16 and all(int(n) == (928 if b == 'tape' else 664) for _, b, n in futures)
        descriptors[arm] = {name: dict((k, int(v)) for k, v in re.findall(r'(\w+)=(\d+)', values)) for name, values in re.findall(r'CO_CASE (\w+) (.*?) tape=', text)}
        counts[arm] = {(name, int(chunk)): (int(dispatch), int(fuel), int(yields)) for name, chunk, dispatch, fuel, yields in re.findall(r'CO_COUNTS (\w+) rows=600 chunk=(\d+) dispatches=(\d+) fuel=(\d+) yields=(\d+)', text)}
        assert len(descriptors[arm]) == 4 and len(counts[arm]) == 8
    for name in ('price', 'quantity'):
        assert descriptors['A'][name] == dict(ops=11, capacity=16, prepared=976, live_capacity=12)
        assert descriptors['B'][name] == dict(ops=7, capacity=8, prepared=560, live_capacity=8)
        for chunk, fuel, yields, dispatch in [(7, 86, 85, 93), (256, 3, 2, 10)]:
            assert counts['A'][name, chunk] == (2407, fuel, yields)
            assert counts['B'][name, chunk] == (dispatch, fuel, yields)
    for name in ('alias', 'unmatched'):
        assert descriptors['A'][name] == descriptors['B'][name]
        for chunk in (7, 256):
            assert counts['A'][name, chunk] == counts['B'][name, chunk]
    machine = load(OUT / 'machine.json.gz')
    machines = {}
    for arm in 'AB':
        assert machine[arm]['binary_sha256'] == manifests['ab']['arms'][arm]['binary_sha256']
        for row in machine[arm]['rows']:
            assert row['instructions'] == len(re.findall(r'^\s*[0-9a-f]+:', row['assembly'], re.M))
            assert row['bytes'] == row['instructions'] * 4
        kernels = [r for r in machine[arm]['rows'] if 'scalar_count' in r['symbol']]
        assert len(kernels) == (0 if arm == 'A' else 2)
        for row in kernels:
            assert row['instructions'] == 20 and row['bytes'] == 80
            assert re.search(r'\bldr\s+x\d+', row['assembly']) and 'b.vs' in row['assembly']
            decoded = '\n'.join(re.findall(r'^\s*[0-9a-f]+:\s+[0-9a-f]{8}\s+(.*)$', row['assembly'], re.M))
            assert not re.search(r'\b[vpqds]\d+\b', decoded)
        dispatch = [r for r in machine[arm]['rows'] if 'run_until_boundary' in r['symbol'] or 'tape3run17h' in r['symbol']]
        assert sum(r['bytes'] for r in dispatch) == (7500 if arm == 'A' else 8084)
        machines[arm] = dict(dispatch_bytes=sum(r['bytes'] for r in dispatch), selected_tape_bytes=sum(r['bytes'] for r in machine[arm]['rows']), binary_bytes=machine[arm]['binary_bytes'])
    verification = load(OUT / 'verification.json.gz')
    assert len(verification['records']) == 9 and all(r['exit_code'] == 0 for r in verification['records'])
    for path, h in verification['sources'].items():
        if path != 'tests/co_count.rs':
            assert digest(source(path, True)) == h
        assert digest((ROOT / path).read_bytes()) == h
    tests = [sum(map(int, re.findall(r'test result: ok\. (\d+) passed', r['stdout']))) for r in verification['records'][:3]]
    summary = dict(base=BASE, decision='keep one bounded pure checked-count descriptor; scalar only; stop CO',
                   processes=288, measured_requests=3456, timing=timing, layouts=layouts,
                   cpu=dict(pre_idle=pre_idle, during_idle=during_idle, online_assessed_samples=len(cpu['samples']), limitation='replacement pair collected two top snapshots; collector omitted final block assessment and failed its postcheck; raw final delta snapshot retained; no further timing or pooling'),
                   descriptors=descriptors, counts={a: {f'{k[0]}/{k[1]}': v for k, v in x.items()} for a,x in counts.items()},
                   machine=machines, future_bytes=dict(tape=928, native=664), release_tests=tests,
                   pmu_measured=False, allocator_census=False, linux_run=False, wasm_run=False)
    (OUT / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    aa = {(c['backend'], c['case']): c['b_over_a'] for c in timing['monitored-aa']['cells']}
    for c in timing['monitored-ab']['cells']:
        print(c['backend'], c['case'], c['medians_ns'], round(c['b_over_a'], 5), round(aa[c['backend'], c['case']], 5))
    print('machine', machines, 'tests default/tool-json/debug', tests)

if __name__ == '__main__':
    main()
