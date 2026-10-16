#!/usr/bin/env python3
"""CY correctness checks, reusing the external target under the shared lock."""
import fcntl
import csv
import hashlib
import json
import os
from pathlib import Path
import subprocess
import statistics
import sys

root = Path(__file__).resolve().parents[1]
os.chdir(root)
out = root / 'bench/results/cy'
out.mkdir(parents=True, exist_ok=True)
env = dict(os.environ, CARGO_TARGET_DIR=str(root / 'target'), CARGO_BUILD_JOBS='1', RUSTUP_AUTO_INSTALL='0')
identity = {'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
            'rustc': subprocess.check_output(['rustc', '+1.96.0', '-Vv'], text=True),
            'platform': subprocess.check_output(['uname', '-a'], text=True),
            'source_sha256': {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted([
                *Path('src').rglob('*.rs'), *Path('tests').glob('*.rs'), *Path('fixtures/cv').glob('*.py'),
                Path('examples/cy_route.rs'), Path('examples/cy_probe.rs'), Path('tools/cy_check.py'), Path('Cargo.lock'), Path('Cargo.toml')])}, 'commands': []}
commands = [
    ('routing', ['test', '--release', '--test', 'cy_routing']),
    ('tool_json', ['test', '--release', '--features', 'tool-json']),
    ('default', ['test', '--release']),
    ('debug_json', ['test', '--release', '--features', 'tool-json,debug-runtime', '--test', 'cy_routing', '--test', 'string_equality']),
    ('example', ['run', '--release', '--example', 'cy_route']),
    ('clippy', ['clippy', '--release', '--features', 'tool-json', '--lib', '--example', 'cy_route', '--example', 'cy_probe', '--test', 'cy_routing', '--', '-D', 'warnings', '-A', 'clippy::type_complexity']),
]
with open('/Users/yuzhe/.hermes/cache/scratch/minipy-perf-build.lock', 'a') as lock:
    fcntl.flock(lock, fcntl.LOCK_EX)
    if '--measure' in sys.argv:
        cmd = ['cargo', '+1.96.0', 'build', '--offline', '--locked', '--release', '--example', 'cy_probe']
        with (out / 'build.txt').open('w') as stream:
            subprocess.run(cmd, env=env, stdout=stream, stderr=subprocess.STDOUT, check=True)
        binary = root / 'target/release/examples/cy_probe'
        identity['commands'].append(cmd)
        identity['binary_sha256'] = hashlib.sha256(binary.read_bytes()).hexdigest()
        identity['load_before'] = os.getloadavg()
        identity['processes_before'] = subprocess.check_output(['ps', '-axo', 'pid,pcpu,comm'], text=True)
        raw = subprocess.check_output([str(binary)], env=env, text=True)
        identity['load_after'] = os.getloadavg()
        identity['commands'].append([str(binary)])
        (out / 'raw.csv').write_text(raw)
        # Keep process names out of the compact identity, retain only CPU-active entries.
        identity['processes_before'] = [line for line in identity['processes_before'].splitlines()[1:] if float(line.split()[1]) >= 5]
        rows = list(csv.DictReader(raw.splitlines()))
        assert len(rows) == 132
        summary = {}
        for backend in ['Tape', 'Native']:
            summary[backend] = {}
            for arm in ['direct', 'route']:
                values = [int(r['ns']) for r in rows if r['backend'] == backend and r['arm'] == arm and int(r['sample']) > 0]
                assert len(values) == 32
                summary[backend][arm] = {'n': len(values), 'median_ns': statistics.median(values)}
            pairs = [{r['arm']: int(r['ns']) for r in rows if r['backend'] == backend and int(r['sample']) == sample} for sample in range(1, 33)]
            summary[backend]['paired_route_over_direct_median'] = statistics.median(p['route'] / p['direct'] for p in pairs)
        (out / 'measure.json').write_text(json.dumps(identity, indent=2) + '\n')
        (out / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
        print(json.dumps(summary, indent=2))
        sys.exit(0)
    for name, args in commands:
        cmd = ['cargo', '+1.96.0', args[0], '--offline', '--locked', *args[1:]]
        with (out / f'{name}.txt').open('w') as stream:
            result = subprocess.run(cmd, env=env, stdout=stream, stderr=subprocess.STDOUT)
        identity['commands'].append({'command': cmd, 'exit': result.returncode})
        (out / 'checks.json').write_text(json.dumps(identity, indent=2) + '\n')
        print(name, result.returncode, flush=True)
        result.check_returncode()
