#!/usr/bin/env python3
"""DA bounded verification/paired diagnostic, shared target and advisory lock."""
import csv
import fcntl
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
os.chdir(ROOT)
OUT = ROOT / 'bench/results/da'
OUT.mkdir(parents=True, exist_ok=True)
ENV = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / 'target'), CARGO_BUILD_JOBS='1', RUSTUP_AUTO_INSTALL='0')
CARGO = ['cargo', '+1.96.0']
identity = {
    'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
    'rustc': subprocess.check_output(['rustc', '+1.96.0', '-Vv'], text=True),
    'platform': subprocess.check_output(['uname', '-a'], text=True),
    'commands': [],
    'source_sha256': {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted([
        *Path('src').rglob('*.rs'), *Path('tests').glob('*.rs'), *Path('fixtures/cv').glob('*.py'),
        Path('examples/da_probe.rs'), Path('tools/da_check.py'), Path('Cargo.toml'), Path('Cargo.lock')])},
}

def run(cmd, name):
    with (OUT / name).open('w') as stream:
        result = subprocess.run(cmd, env=ENV, stdout=stream, stderr=subprocess.STDOUT)
    identity['commands'].append({'command': cmd, 'exit_code': result.returncode, 'log': name})
    print(name, result.returncode, flush=True)
    result.check_returncode()

with open('/Users/yuzhe/.hermes/cache/scratch/minipy-perf-build.lock', 'a') as lock:
    fcntl.flock(lock, fcntl.LOCK_EX)
    try:
        if '--measure' in sys.argv:
            run(CARGO + ['build', '--offline', '--locked', '--release', '--features', 'tool-json', '--example', 'da_probe'], 'build.txt')
            binary = ROOT / 'target/release/examples/da_probe'
            identity['binary_sha256'] = hashlib.sha256(binary.read_bytes()).hexdigest()
            identity['load_before'] = os.getloadavg()
            identity['processes_before'] = subprocess.check_output(['ps', '-axo', 'pid,pcpu,etime,comm'], text=True)
            command = [str(binary)]
            identity['commands'].append({'command': command})
            output = subprocess.check_output(command, env=ENV, text=True)
            identity['load_after'] = os.getloadavg()
            (OUT / 'raw.csv').write_text(output)
            rows = list(csv.DictReader(output.splitlines()))
            assert len(rows) == 132
            summary = {}
            for backend in ['tape', 'native']:
                values = {(r['case'], int(r['sample'])): int(r['ns']) for r in rows if r['backend'] == backend and int(r['sample']) > 0}
                assert len(values) == 64
                summary[backend] = {
                    'pairs': 32,
                    'direct_median_ns': statistics.median(values['direct', i] for i in range(1, 33)),
                    'temporary_median_ns': statistics.median(values['temporary', i] for i in range(1, 33)),
                    'paired_direct_over_temporary_median': statistics.median(values['direct', i] / values['temporary', i] for i in range(1, 33)),
                }
            (OUT / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
            print(json.dumps(summary, indent=2))
        else:
            run(['cargo', '+stable', 'fmt', '--all', '--', '--check'], 'fmt.txt')
            run(CARGO + ['test', '--offline', '--locked', '--release'], 'default.txt')
            targeted = ['--test', 'json_fstring', '--test', 'string_equality', '--test', 'string_tool', '--test', 'cy_routing', '--test', 'tool_json', '--test', 'string_contains']
            for name, feature in [('tool_json', 'tool-json'), ('debug_json', 'tool-json,debug-runtime')]:
                run(CARGO + ['test', '--offline', '--locked', '--release', '--features', feature] + targeted, name + '.txt')
            run(CARGO + ['clippy', '--offline', '--locked', '--release', '--features', 'tool-json,debug-runtime', '--lib', '--test', 'json_fstring', '--test', 'string_equality', '--example', 'da_probe', '--', '-D', 'warnings', '-A', 'clippy::type_complexity'], 'clippy.txt')
            run(['git', 'diff', '--check'], 'diff.txt')
    finally:
        (OUT / ('measure.json' if '--measure' in sys.argv else 'checks.json')).write_text(json.dumps(identity, indent=2) + '\n')
