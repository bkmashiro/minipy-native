#!/usr/bin/env python3
"""Bounded verification with external target and existing exclusive build lock."""
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
from perf_workbench import ROOT, locked

mode, name = sys.argv[1:]
assert mode in ('baseline', 'candidate', 'final')
output = Path(name)
assert not output.exists()
assert os.path.ismount('/Volumes/M.2')
assert (ROOT / 'target').is_symlink()
assert (ROOT / 'target').resolve() == Path('/Volumes/M.2/caches/minipy-native-target')
env = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / 'target'), CARGO_BUILD_JOBS='1',
           CC_CANDIDATE='0' if mode == 'baseline' else '1',
           BZ_CANDIDATE='0' if mode == 'baseline' else '1')
base = ['cargo', '+1.96.0', 'test', '--offline', '--locked', '--release']
commands = [base + ['--features', 'tool-json', '--test', 'cc_waiting', '--', '--nocapture']]
if mode != 'baseline':
    commands += [base, base + ['--features', 'tool-json,debug-runtime'],
                 ['cargo', 'clippy', '--offline', '--locked', '--release', '--all-targets', '--features', 'tool-json,debug-runtime', '--', '-D', 'warnings'],
                 ['cargo', 'fmt', '--check'], ['git', 'diff', '--check']]
rows = []
with locked():
    identity = dict(commit=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
                    host=platform.platform(), rust=subprocess.check_output(['rustc', '+1.96.0', '--version'], text=True).strip(),
                    sources={str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted((ROOT / 'src').rglob('*.rs')) + [ROOT / 'tests/cc_waiting.rs', ROOT / 'tests/bz_detach.rs']})
    for cmd in commands:
        r = subprocess.run(cmd, cwd=ROOT, env=env, capture_output=True, text=True)
        rows.append(dict(command=cmd, exit_code=r.returncode, stdout=r.stdout, stderr=r.stderr))
        print('exit', r.returncode, ' '.join(cmd), flush=True)
        if r.returncode:
            print(r.stdout, r.stderr)
            break
output.parent.mkdir(parents=True, exist_ok=True)
output.write_text(json.dumps(dict(mode=mode, identity=identity, commands=rows), indent=2) + '\n')
assert all(r['exit_code'] == 0 for r in rows)
