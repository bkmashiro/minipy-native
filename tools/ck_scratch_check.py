#!/usr/bin/env python3
"""Serial CK verification with existing shared target/lock, no downloads."""
import json
import os
from pathlib import Path
import subprocess
from perf_workbench import ROOT, locked

commands = [
    ['cargo','+stable','fmt','--all'],
    ['cargo','+1.96.0','test','--offline','--locked','--release'],
    ['cargo','+1.96.0','test','--offline','--locked','--release','--features','tool-json,debug-runtime'],
    ['cargo','+1.96.0','clippy','--offline','--locked','--release','--all-targets','--features','tool-json,debug-runtime','--','-D','warnings'],
    ['cargo','+stable','fmt','--all','--','--check'],
    ['python3','-m','unittest','discover','-s','tools','-p','test_perf_workbench.py','-v'],
    ['git','diff','--check'],
]
if __name__ == '__main__':
    out=ROOT/'evidence/ck-scratch/verification.json'
    if out.exists(): raise ValueError('refuse overwrite')
    env=dict(os.environ,CARGO_BUILD_JOBS='1',RUSTUP_AUTO_INSTALL='0',CARGO_TARGET_DIR=str(ROOT/'target'))
    assert os.path.ismount('/Volumes/M.2') and (ROOT/'target').resolve()==Path('/Volumes/M.2/caches/minipy-native-target')
    rows=[]
    with locked():
        for cmd in commands:
            r=subprocess.run(cmd,cwd=ROOT,env=env,text=True,capture_output=True)
            rows.append(dict(command=cmd,exit_code=r.returncode,stdout=r.stdout,stderr=r.stderr))
            print(cmd,r.returncode,flush=True)
            if r.returncode:
                print(r.stdout,r.stderr)
                break
    out.parent.mkdir(parents=True,exist_ok=True)
    out.write_text(json.dumps(rows,indent=2)+'\n')
    if rows[-1]['exit_code']: raise SystemExit(1)
