#!/usr/bin/env python3
"""Serialize feasibility tests and bounded cost floor; fixed binary, no profiles."""
import fcntl
import gzip
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'evidence/cb-copy-patch'
SCRATCH = Path.home() / '.hermes/cache/scratch'

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def reduce(path):
    with gzip.open(path, 'rt') as f:
        rows = [json.loads(s) for s in f]
    assert rows[0]['kind'] == 'manifest' and rows[-1] == {'kind': 'end', 'failures': 0}
    groups = {}
    processes = rows[1:-1]
    aa = rows[0].get('aa', False)
    expected_processes = 8 if aa else 24
    assert len(processes) == expected_processes and len({(r['round'], r['slot'], r['mode']) for r in processes}) == expected_processes
    for row in processes:
        assert row['exit_code'] == 0
        samples = row['samples']
        assert [s['sample'] for s in samples] == list(range(65))
        assert all(type(s['ns']) is int and s['ns'] > 0 for s in samples)
        groups.setdefault(row['mode'], []).append(statistics.median(s['ns'] for s in samples[1:]))
    medians = {mode: statistics.median(values) for mode, values in groups.items()}
    assert all(len(values) == 8 for values in groups.values())
    if aa:
        arms = {arm: [statistics.median(s['ns'] for s in r['samples'][1:]) for r in processes if r['arm'] == arm] for arm in ('A', 'B')}
        return {'processes': len(processes), 'measured_requests': len(processes)*64,
                'process_medians_ns': arms, 'b_over_a': statistics.median(arms['B'])/statistics.median(arms['A']),
                'note': 'Same fixed executable and tape path in both arms, ABBA twice.'}
    return {'processes': len(processes), 'measured_requests': len(processes)*64,
            'process_medians_ns': groups, 'arm_medians_ns': medians,
            'floor_over_full_tape': medians['install_floor']/medians['tape'],
            'floor_over_full_native': medians['install_floor']/medians['native'],
            'note': 'Optimistic raw install floor, not a source-to-source backend speedup. Sample 0 excluded.'}

def main():
    aa = '--aa' in sys.argv
    raw = OUT / ('aa.jsonl.gz' if aa else 'cost-gate.jsonl.gz')
    if raw.exists():
        raise ValueError('refuse to overwrite raw')
    target = ROOT / 'target'
    assert os.path.ismount('/Volumes/M.2') and target.is_symlink()
    assert target.resolve() == Path('/Volumes/M.2/caches/minipy-native-target')
    env = dict(os.environ, CARGO_BUILD_JOBS='1', CARGO_TARGET_DIR=str(target))
    binary = None
    with (SCRATCH / 'minipy-perf-build.lock').open('a+') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        cmd = ['cargo', '+1.96.0', 'test', '--offline', '--locked', '--release', '--test', 'cb_gate', '--no-run', '--message-format=json']
        result = subprocess.run(cmd, env=env, cwd=ROOT, capture_output=True, text=True)
        (OUT / 'build.txt').write_text(result.stderr)
        if result.returncode:
            print(result.stdout, result.stderr)
            raise ValueError('build failed')
        artifacts = [json.loads(s) for s in result.stdout.splitlines() if s.startswith('{')]
        binaries = [r['executable'] for r in artifacts if r.get('reason') == 'compiler-artifact' and r.get('target', {}).get('name') == 'cb_gate' and r.get('executable')]
        assert len(binaries) == 1
        binary = Path(binaries[0])
        digest = sha(binary)
        result = subprocess.run([str(binary), '--test-threads=1'], env=env, capture_output=True, text=True, cwd=ROOT)
        (OUT / 'correctness.txt').write_text((result.stdout + result.stderr).rstrip() + '\n')
        assert result.returncode == 0
        manifest = {'kind': 'manifest', 'base_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
                    'build_command': cmd, 'binary_sha256': digest, 'host': platform.platform(),
                    'rust': subprocess.check_output(['rustc', '+1.96.0', '--version'], text=True).strip(),
                    'sources': {str(p.relative_to(ROOT)): sha(p) for p in sorted((ROOT/'src').rglob('*.rs')) + [ROOT/'Cargo.toml', ROOT/'Cargo.lock', ROOT/'tests/cb_gate.rs', OUT/'templates.rs', OUT/'stencils.c']},
                    'reps': 64, 'rounds': 2, 'order': 'ABBA mode order reverses by round',
                    'aa': aa, 'modes': ['tape'] if aa else ['tape', 'install_floor', 'native']}
        failures = 0
        with gzip.open(raw, 'xt') as f:
            f.write(json.dumps(manifest)+'\n')
            for rnd in range(2):
                for slot in range(4):
                    order = manifest['modes'] if (slot in (0, 3)) != bool(rnd) else list(reversed(manifest['modes']))
                    for mode in order:
                        command = [str(binary), '--ignored', '--exact', 'cost_gate', '--nocapture', '--test-threads=1']
                        result = subprocess.run(command, env=dict(env, CB_MODE=mode, CB_REPS='64'), cwd=ROOT, capture_output=True, text=True, timeout=60)
                        samples = [json.loads(s.partition('CB ')[2]) for s in result.stdout.splitlines() if 'CB ' in s]
                        failures += result.returncode != 0
                        row = {'kind':'process', 'round':rnd, 'slot':slot, 'arm':'ABBA'[slot], 'mode':mode, 'samples':samples, 'command':command, 'exit_code':result.returncode}
                        if result.returncode: row['error'] = result.stdout + result.stderr
                        f.write(json.dumps(row)+'\n'); f.flush()
                        assert sha(binary) == digest
                        print(rnd, slot, mode, result.returncode, flush=True)
            f.write(json.dumps({'kind':'end', 'failures':failures})+'\n')
        result = reduce(raw)
        (OUT / ('aa-summary.json' if aa else 'summary.json')).write_text(json.dumps(result, indent=2)+'\n')
        # Cargo may rebuild it next time; do not retain the experimental executable.
        binary.unlink()
    print(json.dumps(result, indent=2))

if __name__ == '__main__':
    main()
