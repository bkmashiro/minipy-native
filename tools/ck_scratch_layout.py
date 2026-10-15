#!/usr/bin/env python3
"""Separate candidate-only layout replay, reusing CI test overlay and fixtures."""
import gzip
import json
import os
from pathlib import Path
import subprocess
from perf_workbench import ROOT, locked, sha
OUT=ROOT/'evidence/ck-scratch'
if __name__=='__main__':
    output=OUT/'layout.json.gz'
    if output.exists(): raise ValueError('refuse overwrite')
    patch=ROOT/'evidence/ci-layout/diagnostic.patch'
    env=dict(os.environ,CARGO_BUILD_JOBS='1',RUSTUP_AUTO_INSTALL='0',CARGO_TARGET_DIR=str(ROOT/'target'))
    rows=[]
    def run(cmd):
        r=subprocess.run(cmd,cwd=ROOT,env=env,text=True,capture_output=True)
        rows.append(dict(command=cmd,exit_code=r.returncode,stdout=r.stdout,stderr=r.stderr))
        if r.returncode: print(r.stdout,r.stderr)
        r.check_returncode()
        return r.stdout
    with locked():
        assert os.path.ismount('/Volumes/M.2') and (ROOT/'target').resolve()==Path('/Volumes/M.2/caches/minipy-native-target')
        identity={str(p.relative_to(ROOT)):sha(p) for p in (ROOT/'src').rglob('*.rs')}
        run(['git','apply',str(patch)])
        try:
            build=run(['cargo','+1.96.0','test','--offline','--locked','--release','--features','tool-json','--lib','--no-run','--message-format=json'])
            artifacts=[json.loads(s) for s in build.splitlines() if s.startswith('{')]
            binaries=[r['executable'] for r in artifacts if r.get('executable') and r.get('target',{}).get('name')=='minipy_native']
            assert len(binaries)==1
            rows[-1]['stdout']=json.dumps([r for r in artifacts if r.get('executable')])
            run([binaries[0],'--ignored','--exact','interpreter::ci_probe::ci_layout','--nocapture','--test-threads=1'])
            run([binaries[0],'interpreter::scratch_tests','--nocapture','--test-threads=1'])
            raw=dict(sources=identity,overlay_sha256=sha(patch),binary_sha256=sha(binaries[0]),records=rows)
        finally:
            run(['git','apply','--reverse',str(patch)])
            assert all(sha(ROOT/p)==h for p,h in identity.items())
        with gzip.open(output,'xt') as f: json.dump(raw,f,indent=2)
    print('saved',output)
