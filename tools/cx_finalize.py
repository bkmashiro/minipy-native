#!/usr/bin/env python3
"""Reconcile CX evidence without resampling or replacing either timing batch."""
import csv
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import statistics
import subprocess

root = Path(__file__).resolve().parents[1]
os.chdir(root)
out = root / 'bench/results/cx'
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
identity = json.loads((out / 'check_identity.json').read_text())
assert all(sha(p) == h for p, h in identity['source_sha256'].items())
report = {'reused_suites': {}, 'commands': [], 'timing_status': 'Both batches diagnostic only; concurrent workers, not low-load evidence. Never pooled.'}
for name in ['default', 'tool_json', 'debug_json']:
    text = (out / f'{name}.txt').read_text()
    counts = re.findall(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored', text)
    assert counts and 'test result: FAILED' not in text
    report['reused_suites'][name] = {'passed': sum(int(a) for a, _ in counts), 'ignored': sum(int(b) for _, b in counts)}
for path in sorted(Path('fixtures/cv').glob('*.py')):
    original = Path('/Users/yuzhe/.hermes/profiles/akashi/reports/minipy-native/cv-t_867c46df/cases') / path.name
    assert original.read_bytes() == path.read_bytes(), path
report['unchanged_cv_fixtures'] = 10
rows = list(csv.DictReader((out / 'raw.csv').open()))
assert len(rows) == 594
summary = json.loads((out / 'summary.json').read_text())
for backend in ['tape', 'native']:
    for case in ['eq', 'contains', 'scalar']:
        values = [int(r['ns']) for r in rows if r['backend'] == backend and r['case'] == case and int(r['sample']) > 0]
        assert len(values) == 96
        assert summary[backend][case] == {'n': 96, 'median_ns': statistics.median(values)}
    ratios = []
    for process in range(3):
        for sample in range(1, 33):
            pair = {r['case']: int(r['ns']) for r in rows if int(r['process']) == process and r['backend'] == backend and int(r['sample']) == sample}
            ratios.append(pair['eq'] / pair['contains'])
    assert summary[backend]['paired_eq_over_contains_median'] == statistics.median(ratios)
report['timing_batches'] = {'raw.csv': {'records': len(rows), 'measured': 576}, 'run-*.csv': {'records': sum(len(list(csv.DictReader(p.open()))) for p in out.glob('run-*.csv'))}}
report['source_sha256'] = {str(p): sha(p) for p in sorted([*Path('src').rglob('*.rs'), *Path('tests').glob('*.rs'), *Path('fixtures/cv').glob('*.py'), Path('Cargo.toml'), Path('Cargo.lock'), Path('examples/cx_probe.rs'), Path('tools/cx_check.py'), Path('tools/cx_finalize.py')])}
env = dict(os.environ, CARGO_TARGET_DIR=str(root / 'target'), CARGO_BUILD_JOBS='1', RUSTUP_AUTO_INSTALL='0')
assert (root / 'target').resolve() == Path('/Volumes/M.2/caches/minipy-native-target')
commands = [
    ['cargo', '+1.96.0', 'test', '--offline', '--locked', '--release', '--features', 'tool-json', '--test', 'string_equality', '--test', 'string_contains', '--test', 'string_tool', '--test', 'local_read_host'],
    ['cargo', '+1.96.0', 'test', '--offline', '--locked', '--release', '--features', 'tool-json,debug-runtime', '--test', 'string_equality'],
    # Existing CT annotation at frontend.rs:195 triggers this lint on 1.96.0.
    ['cargo', '+1.96.0', 'clippy', '--offline', '--locked', '--lib', '--test', 'string_equality', '--example', 'cx_probe', '--features', 'tool-json,debug-runtime', '--', '-D', 'warnings', '-A', 'clippy::type_complexity'],
    ['cargo', '+stable', 'fmt', '--all', '--', '--check'],
    ['git', 'diff', '--check'],
]
with open('/Users/yuzhe/.hermes/cache/scratch/minipy-perf-build.lock', 'a') as lock:
    fcntl.flock(lock, fcntl.LOCK_EX)
    for i, cmd in enumerate(commands):
        log = out / f'final-{i}.txt'
        with log.open('w') as stream:
            result = subprocess.run(cmd, env=env, stdout=stream, stderr=subprocess.STDOUT)
        report['commands'].append({'argv': cmd, 'exit_code': result.returncode, 'log': str(log)})
        print(log, result.returncode, flush=True)
        if result.returncode:
            print(log.read_text())
            break
report['evidence_sha256'] = {str(p): sha(p) for p in sorted(out.iterdir()) if p.is_file() and p.name != 'reconciliation.json'}
(out / 'reconciliation.json').write_text(json.dumps(report, indent=2) + '\n')
assert all(c['exit_code'] == 0 for c in report['commands'])
