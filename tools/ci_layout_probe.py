#!/usr/bin/env python3
"""Separate CI diagnosis; apply evidence/ci-layout/diagnostic.patch first."""
import json
import argparse
import re
import os
from pathlib import Path
import platform
import subprocess
from perf_workbench import ROOT, locked, sha

OUT = ROOT / 'evidence/ci-layout'

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=OUT / 'diagnostic-executed.json')
    args = parser.parse_args()
    raw = args.output
    if raw.exists():
        raise ValueError('refuse to overwrite diagnosis')
    if not os.path.ismount('/Volumes/M.2') or (ROOT / 'target').resolve() != Path('/Volumes/M.2/caches/minipy-native-target'):
        raise ValueError('expected mounted external shared target')
    records = []
    env = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / 'target'), CARGO_BUILD_JOBS='1', RUSTUP_AUTO_INSTALL='0')
    def run(cmd):
        r = subprocess.run(cmd, cwd=ROOT, env=env, text=True, capture_output=True)
        records.append(dict(command=cmd, exit_code=r.returncode, stdout=r.stdout, stderr=r.stderr))
        return r
    with locked():
        meta: dict = dict(commit=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),
                    host=platform.platform(), rust=subprocess.check_output(['rustc','+1.96.0','--version'],text=True).strip(),
                    sources={str(p.relative_to(ROOT)):sha(p) for p in sorted((ROOT/'src').rglob('*.rs'))},
                    probe_sha256=sha(OUT/'probe.rs'), lock=str(Path.home()/'.hermes/cache/scratch/minipy-perf-build.lock'))
        build = run(['cargo','+1.96.0','test','--offline','--locked','--release','--features','tool-json','--lib','--no-run','--message-format=json'])
        binary = None
        if build.returncode == 0:
            artifacts=[json.loads(s) for s in build.stdout.splitlines() if s.startswith('{')]
            binaries=[r['executable'] for r in artifacts if r.get('executable') and r.get('target',{}).get('name')=='minipy_native']
            assert len(binaries)==1
            binary=binaries[0]
            records[0]['stdout'] = json.dumps([r for r in artifacts if r.get('executable')])
            meta['binary_sha256']=sha(binary)
            for test in ['interpreter::ci_probe::ci_layout','codegen::tests::ci_native']:
                run([binary,'--ignored','--exact',test,'--nocapture','--test-threads=1'])
            objdump='/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin/llvm-objdump'
            syms=subprocess.check_output([objdump,'--syms',binary],text=True)
            selected=sorted({s.split()[-1] for s in syms.splitlines() if ' F ' in s and 'interpreter' in s and ('run_until_boundary' in s or '4tape3run' in s or '3run17' in s) and 'debug' not in s})
            meta['tape_symbols']=selected
            if selected:
                asm=run([objdump,'--disassemble-symbols='+','.join(selected),binary])
                # Preserve bytes/offsets, shorten repeated Rust symbol expansions only.
                text = asm.stdout
                for symbol in selected:
                    text = text.replace(symbol, 'tape_run_until_boundary')
                (OUT/'tape-aarch64.txt').write_text(text)
                if len(re.findall(r'^\s*[0-9a-f]+:', text, re.M)) < 10:
                    raise ValueError('objdump returned no instructions')
                records[-1]['stdout']='Saved in tape-aarch64.txt'
            # Run the whole lib suite with the overlay, excluding ignored diagnostics.
            run([binary,'--test-threads=1'])
            run(['cargo','+1.96.0','test','--offline','--locked','--release','--features','tool-json,interpreter-profile',
                 '--lib','ci_loop_counts','--','--ignored','--nocapture','--test-threads=1'])
        raw.write_text(json.dumps(dict(identity=meta,records=records),indent=2)+'\n')
    print(json.dumps(dict(build_ok=build.returncode==0, binary=binary,
                         exits=[r['exit_code'] for r in records],symbols=meta.get('tape_symbols')),indent=2))
    if any(r['exit_code'] for r in records):
        for r in records:
            if r['exit_code']:
                print(r['stderr'][-6000:],r['stdout'][-6000:])
        raise SystemExit(1)

if __name__=='__main__':
    main()
