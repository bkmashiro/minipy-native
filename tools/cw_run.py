#!/usr/bin/env python3
"""Build/capture one CW arm under the shared advisory lock. No new target tree."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

arm = sys.argv[1]
root = Path(__file__).resolve().parents[1]
os.chdir(root)
out = root / 'bench/results/cw'
out.mkdir(parents=True, exist_ok=True)
env = dict(os.environ, CARGO_TARGET_DIR=str(root / 'target'), CARGO_BUILD_JOBS='1', RUSTUP_AUTO_INSTALL='0')
with open('/Users/yuzhe/.hermes/cache/scratch/minipy-perf-build.lock', 'a') as lock:
    fcntl.flock(lock, fcntl.LOCK_EX)
    identity = {'arm': arm, 'head': subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),
                'frontend_sha256': hashlib.sha256((root/'src/frontend.rs').read_bytes()).hexdigest(),
                'rustc': subprocess.check_output(['rustc','+1.96.0','-Vv'],text=True), 'builds': {}}
    for mode, features in [('speed','tool-json'), ('alloc','json-allocation-profile')]:
        cmd = ['cargo','+1.96.0','build','--offline','--locked','--release','--example','cw_probe','--features',features]
        subprocess.run(cmd, env=env, check=True)
        binary = root / 'target/release/examples/cw_probe'
        saved = root / 'target/release/examples' / f'cw_{arm}_{mode}'
        import shutil
        shutil.copy2(binary, saved)
        identity['builds'][mode] = {'command': cmd, 'binary': str(saved), 'sha256': hashlib.sha256(saved.read_bytes()).hexdigest()}
        if mode == 'alloc':
            with (out/f'{arm}_alloc.csv').open('w') as f:
                f.write('case,native,iteration,ns,calls,bytes\n')
                f.flush()
                for backend in ['tape','native']:
                    for case in ['no_int','short','dense']:
                        subprocess.run([str(saved),backend,case,'3'],stdout=f,check=True)
    (out/f'{arm}_identity.json').write_text(json.dumps(identity,indent=2)+'\n')
