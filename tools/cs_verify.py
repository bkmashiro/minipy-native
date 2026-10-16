#!/usr/bin/env python3
"""Bounded CS integration checks, no performance collection."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'evidence/cs-integration/verification.json'


def main():
    assert not OUT.exists(), 'refuse to overwrite verification'
    assert os.path.ismount('/Volumes/M.2')
    assert (ROOT / 'target').is_symlink()
    assert (ROOT / 'target').resolve() == Path('/Volumes/M.2/caches/minipy-native-target')
    env = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / 'target'),
               CARGO_BUILD_JOBS='1', RUSTUP_AUTO_INSTALL='0')
    assert not env.get('RUSTFLAGS') and not env.get('CARGO_ENCODED_RUSTFLAGS')
    commands = []
    for features in ('', 'tool-json', 'tool-json,debug-runtime'):
        cmd = ['cargo', '+1.96.0', 'test', '--offline', '--locked', '--release']
        if features:
            cmd += ['--features', features]
        commands.append(cmd)
    commands += [
        ['cargo', '+stable', 'fmt', '--all', '--', '--check'],
        ['python3', '-m', 'unittest', 'discover', '-s', 'tools', '-p', 'test_perf_workbench.py', '-v'],
        ['python3', 'tools/co_count_reduce.py'],
    ]
    paths = subprocess.check_output(['git', 'ls-files', 'src', 'tests', 'Cargo.toml', 'Cargo.lock'], cwd=ROOT, text=True).splitlines()
    result: dict = dict(platform=os.uname().sysname, machine=os.uname().machine,
                  source_sha256={p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in paths},
                  rustc=subprocess.check_output(['rustup', 'run', '1.96.0', 'rustc', '-Vv'], text=True),
                  new_performance_samples=0, records=[])
    original = (ROOT / 'evidence/co-count/summary.json').read_bytes()
    with open(Path.home() / '.hermes/cache/scratch/minipy-perf-build.lock', 'a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        for cmd in commands:
            start = time.monotonic()
            p = subprocess.run(cmd, cwd=ROOT, env=env, text=True, capture_output=True)
            row = dict(command=cmd, exit_code=p.returncode, seconds=time.monotonic() - start,
                       stdout=p.stdout, stderr=p.stderr)
            result['records'].append(row)
            print(json.dumps(dict(command=cmd, exit_code=p.returncode,
                                  passed=sum(map(int, re.findall(r'test result: ok\. (\d+) passed', p.stdout))))), flush=True)
            OUT.parent.mkdir(parents=True, exist_ok=True)
            OUT.write_text(json.dumps(result, indent=2) + '\n')
            if p.returncode:
                print(p.stdout, p.stderr)
                raise SystemExit(p.returncode)
    assert (ROOT / 'evidence/co-count/summary.json').read_bytes() == original
    print('CO summary replay byte-identical; all checks passed', flush=True)


if __name__ == '__main__':
    main()
