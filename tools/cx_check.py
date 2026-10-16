#!/usr/bin/env python3
"""Bounded CX checks/probe using the existing target and shared advisory lock."""
import csv
import fcntl
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys

root = Path(__file__).resolve().parents[1]
os.chdir(root)
out = root / 'bench/results/cx'
out.mkdir(parents=True, exist_ok=True)
env = dict(os.environ, CARGO_TARGET_DIR=str(root / 'target'), CARGO_BUILD_JOBS='1', RUSTUP_AUTO_INSTALL='0')
cargo = ['cargo', '+1.96.0']
identity = {'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
            'rustc': subprocess.check_output(['rustc', '+1.96.0', '-Vv'], text=True),
            'platform': subprocess.check_output(['uname', '-a'], text=True), 'commands': [],
            'source_sha256': {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in
                              sorted([*Path('src').rglob('*.rs'), *Path('fixtures/cv').glob('*.py'), Path('tests/string_equality.rs'), Path('examples/cx_probe.rs'), Path('tools/cx_check.py'), Path('Cargo.lock'), Path('Cargo.toml')])}}

def run(cmd, dest):
    identity['commands'].append(cmd)
    with dest.open('w') as stream:
        result = subprocess.run(cmd, env=env, stdout=stream, stderr=subprocess.STDOUT)
    print(dest, result.returncode, flush=True)
    result.check_returncode()

with open('/Users/yuzhe/.hermes/cache/scratch/minipy-perf-build.lock', 'a') as lock:
    fcntl.flock(lock, fcntl.LOCK_EX)
    if '--measure' in sys.argv:
        run(cargo + ['build', '--offline', '--locked', '--release', '--features', 'tool-json', '--example', 'cx_probe'], out / 'build.txt')
        binary = root / 'target/release/examples/cx_probe'
        identity['binary_sha256'] = hashlib.sha256(binary.read_bytes()).hexdigest()
        rows = []
        for process in range(3):
            identity.setdefault('load', []).append({'process': process, 'before': os.getloadavg()})
            cmd = [str(binary)]
            identity['commands'].append(cmd)
            result = subprocess.check_output(cmd, env=env, text=True)
            parsed = list(csv.DictReader(result.splitlines()))
            assert len(parsed) == 198
            for row in parsed:
                rows.append(dict(process=process, **row))
            identity['load'][-1]['after'] = os.getloadavg()
        with (out / 'raw.csv').open('w') as stream:
            writer = csv.DictWriter(stream, fieldnames=['process', 'backend', 'case', 'sample', 'ns'])
            writer.writeheader()
            writer.writerows(rows)
        summary = {}
        for backend in ['tape', 'native']:
            summary[backend] = {}
            for case in ['eq', 'contains', 'scalar']:
                values = [int(r['ns']) for r in rows if r['backend'] == backend and r['case'] == case and int(r['sample']) > 0]
                assert len(values) == 96
                summary[backend][case] = {'n': len(values), 'median_ns': statistics.median(values)}
            summary[backend]['paired_eq_over_contains_median'] = statistics.median([
                int(next(r['ns'] for r in rows if int(r['process']) == process and r['backend'] == backend and r['case'] == 'eq' and int(r['sample']) == sample)) /
                int(next(r['ns'] for r in rows if int(r['process']) == process and r['backend'] == backend and r['case'] == 'contains' and int(r['sample']) == sample))
                for process in range(3) for sample in range(1, 33)])
        (out / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
        (out / 'measure_identity.json').write_text(json.dumps(identity, indent=2) + '\n')
        print(json.dumps(summary, indent=2))
    else:
        for name, features in [('default', []), ('tool_json', ['--features', 'tool-json']), ('debug_json', ['--features', 'tool-json,debug-runtime'])]:
            run(cargo + ['test', '--offline', '--locked'] + features, out / f'{name}.txt')
        (out / 'check_identity.json').write_text(json.dumps(identity, indent=2) + '\n')
