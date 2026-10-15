#!/usr/bin/env python3
"""Serial bounded verification/ownership diagnosis under the existing perf lock."""
import json
import os
from pathlib import Path
import subprocess
import sys
from perf_workbench import ROOT, locked

output = Path(sys.argv[1])
mode = sys.argv[2]
assert mode in ('candidate', 'candidate-diagnostic', 'baseline', 'final')
assert not output.exists()
env = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / 'target'), CARGO_BUILD_JOBS='1', BZ_CANDIDATE='1' if mode.startswith('candidate') else '0')
base = ['cargo', '+1.96.0', 'test', '--offline', '--locked', '--release']
if mode in ('baseline', 'candidate-diagnostic'):
    commands = [base + ['--features', 'tool-json', '--test', 'bz_detach', 'root_detachment_diagnostic', '--', '--ignored', '--nocapture']]
elif mode == 'candidate':
    commands = [base, base + ['--features', 'tool-json,debug-runtime'], base + ['--features', 'tool-json', '--test', 'bz_detach', 'root_detachment_diagnostic', '--', '--ignored', '--nocapture']]
else:
    commands = [base, base + ['--features', 'tool-json,debug-runtime'], ['cargo', 'clippy', '--offline', '--locked', '--release', '--all-targets', '--features', 'tool-json,debug-runtime', '--', '-D', 'warnings'], ['cargo', 'fmt', '--check'], ['python3', '-m', 'unittest', 'discover', '-s', 'tools', '-p', 'test_perf_workbench.py', '-v'], ['git', 'diff', '--check']]
rows = []
with locked():
    for cmd in commands:
        r = subprocess.run(cmd, cwd=ROOT, env=env, capture_output=True, text=True)
        rows.append(dict(command=cmd, exit_code=r.returncode, stdout=r.stdout, stderr=r.stderr))
        print('exit', r.returncode, ' '.join(cmd), flush=True)
        if r.returncode:
            print(r.stdout, r.stderr)
            break
output.parent.mkdir(parents=True, exist_ok=True)
output.write_text(json.dumps(dict(mode=mode, commands=rows), indent=2) + '\n')
assert all(r['exit_code'] == 0 for r in rows)
