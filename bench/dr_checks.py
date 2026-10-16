#!/usr/bin/env python3
"""Bounded DR checks in the existing, mounted target. No dependency downloads."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
ROOT = Path(os.environ.get('DR_ROOT', Path(__file__).resolve().parents[1]))
assert os.path.ismount('/Volumes/M.2')
assert (ROOT / 'target').is_symlink() and (ROOT / 'target').resolve() == Path('/Volumes/M.2/caches/minipy-native-target')
env = dict(os.environ, CARGO_TARGET_DIR=str(ROOT/'target'), CARGO_BUILD_JOBS='1', RUSTUP_AUTO_INSTALL='0')
command = ['cargo', '+1.96.0'] + sys.argv[2:]
with open('/Users/yuzhe/.hermes/cache/scratch/minipy-perf-build.lock', 'a') as lock:
    fcntl.flock(lock, fcntl.LOCK_EX)
    result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True)
    output = dict(command=command, returncode=result.returncode, stdout=result.stdout, stderr=result.stderr,
        head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
        rustc=subprocess.check_output(['rustc','+1.96.0','-Vv'],text=True),
        sources={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for pattern in ['src/**/*.rs','tests/line_scan.rs','tests/dr_cost.rs','Cargo.toml','Cargo.lock'] for p in ROOT.glob(pattern)})
    path=ROOT/'bench/results/dr'/sys.argv[1]
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(output,indent=2)+'\n')
    print(json.dumps({k:v for k,v in output.items() if k not in ['sources','rustc']}))
sys.exit(result.returncode)
