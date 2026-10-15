#!/usr/bin/env python3
"""Collect CK machine code from the exact formal binaries, under the shared lock."""
import gzip
import json
import re
import subprocess
from pathlib import Path
from perf_workbench import ROOT, SCRATCH, locked, sha

OUT=ROOT/'evidence/ck-scratch'
OBJDUMP='/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin/llvm-objdump'
if __name__=='__main__':
    output=OUT/'machine.json.gz'
    if output.exists(): raise ValueError('refuse overwrite')
    raw={}
    with locked():
        for arm in 'AB':
            binary=SCRATCH/f'ck-{arm.lower()}'
            syms=subprocess.check_output([OBJDUMP,'--syms',str(binary)],text=True)
            selected=sorted({s.split()[-1] for s in syms.splitlines() if ' F ' in s and (
                'interpreter4tape18run_until_boundary' in s or 'interpreter4tape3run17' in s)})
            assert len(selected)==(1 if arm=='A' else 2),selected
            sections={}
            for symbol in selected:
                cmd=[OBJDUMP,'--disassemble-symbols='+symbol,str(binary)]
                r=subprocess.run(cmd,text=True,capture_output=True,check=True)
                sections[symbol]={'command':cmd,'exit_code':r.returncode,'stdout':r.stdout,'stderr':r.stderr,
                                  'instructions':len(re.findall(r'^\s*[0-9a-f]+:',r.stdout,re.M))}
            cmd=[OBJDUMP,'--macho','--indirect-symbols',str(binary)]
            imports=subprocess.check_output(cmd,text=True)
            raw[arm]={'binary_sha256':sha(binary),'symbols':selected,'sections':sections,
                      'imports_command':cmd,'zero_imports':[s for s in imports.splitlines() if '_bzero' in s or '_memset' in s]}
    with gzip.open(output,'xt') as f: json.dump(raw,f,indent=2)
    for arm,data in raw.items():
        for symbol,section in data['sections'].items():
            print(arm,symbol,section['instructions']);print('\n'.join(section['stdout'].splitlines()[:80]))
