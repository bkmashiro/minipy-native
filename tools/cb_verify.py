#!/usr/bin/env python3
"""Focused offline verification, shared lock/target, no additional timing."""
import fcntl
import json
import os
from pathlib import Path
import subprocess
from cb_gate import reduce, OUT, ROOT, SCRATCH

with (SCRATCH / 'minipy-perf-build.lock').open('a+') as lock:
    fcntl.flock(lock, fcntl.LOCK_EX)
    target = ROOT / 'target'
    assert os.path.ismount('/Volumes/M.2') and target.is_symlink()
    assert target.resolve() == Path('/Volumes/M.2/caches/minipy-native-target')
    env = dict(os.environ, CARGO_BUILD_JOBS='1', CARGO_TARGET_DIR=str(target))
    commands = [
        ['cargo', '+1.96.0', 'test', '--offline', '--locked', '--release', '--test', 'cb_gate'],
        ['cargo', 'clippy', '--offline', '--locked', '--test', 'cb_gate', '--', '-D', 'warnings'],
        ['rustfmt', '--check', '--edition', '2024', 'tests/cb_gate.rs'],
        ['git', 'diff', '--check'],
    ]
    rows = []
    for command in commands:
        result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True)
        rows.append({'command': command, 'exit_code': result.returncode, 'output': result.stdout + result.stderr})
        print(command, result.returncode, flush=True)
        if result.returncode: break
    original = subprocess.check_output(['git', 'diff', '0a23fac', '--', 'src', 'Cargo.toml', 'Cargo.lock'], cwd=ROOT, text=True)
    assert not original
    reductions = {name: reduce(OUT/name) for name in ('cost-gate.jsonl.gz', 'aa.jsonl.gz')}
    for raw, summary in [('cost-gate.jsonl.gz', 'summary.json'), ('aa.jsonl.gz', 'aa-summary.json')]:
        assert reductions[raw] == json.loads((OUT/summary).read_text())
    # Retained examples exercise unsupported object identities without executing bytes.
    from cb_templates import extract
    rejected = 0
    for data in (bytes(32), b'\xcf\xfa\xed\xfe' + bytes(28)):
        try: extract(data)
        except ValueError: rejected += 1
    assert rejected == 2
    (OUT/'verification.json').write_text(json.dumps({'commands': rows, 'production_diff_empty': True, 'raw_reductions_match': True, 'malformed_object_identities_rejected': rejected}, indent=2)+'\n')
    # Remove only gate executables produced during focused verification.
    removed = []
    for p in (target/'release/deps').glob('cb_gate-*'):
        if p.is_file() and os.access(p, os.X_OK):
            removed.append({'name':p.name, 'bytes':p.stat().st_size}); p.unlink()
    print(json.dumps({'removed_gate_binaries': removed}))
    assert all(row['exit_code'] == 0 for row in rows)
