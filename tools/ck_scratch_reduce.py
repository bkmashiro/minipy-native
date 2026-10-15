#!/usr/bin/env python3
"""Replay CK fixed-source request and actual machine-code evidence. No build/timing."""
import gzip
import hashlib
import json
from pathlib import Path
import re
import subprocess
from ce_overflow_reduce import apply
from perf_workbench import reduce
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'evidence/ck-scratch'
BASE='049b8102509d2d687912f0898ef1eda63bb81b14'
def digest(data): return hashlib.sha256(data).hexdigest()
def load(path):
    with gzip.open(path,'rt') as f: return json.load(f)
def main():
    with gzip.open(OUT/'candidate.patch.gz','rt') as f: patch_text=f.read()
    patches={p.splitlines()[0].split()[1][2:]:p for p in patch_text.split('diff --git ')[1:]}
    assert set(patches)=={'src/interpreter.rs','src/interpreter/tape.rs','src/interpreter/scratch_tests.rs','tests/ck_scratch.rs'}
    added={'src/interpreter/scratch_tests.rs','tests/ck_scratch.rs'}
    def source(path,candidate):
        if path in added:
            assert candidate
            return ''.join(s[1:]+'\n' for s in patches[path].splitlines() if s.startswith('+') and not s.startswith('+++')).encode()
        data=subprocess.check_output(['git','show',f'{BASE}:{path}'],cwd=ROOT).decode()
        return (apply(data,patches[path]) if candidate and path in patches else data).encode()
    manifests={};timing={}
    for name in ['ab','aa']:
        path=OUT/f'{name}.jsonl.gz'
        result=reduce(path)
        assert result['complete'] and result['observed_processes']==40 and result['measured_samples']==960
        with gzip.open(path,'rt') as f: m=json.loads(next(f))
        assert m['rounds']==1 and m['reps']==24 and not m['diagnostic']
        assert m['entries']=={'A':'registered','B':'registered'}
        assert m['driver_sha256']==digest((ROOT/'tools/perf_workbench.py').read_bytes())
        assert {tuple(c) for c in m['cells']}=={(b,c) for b in ['tape','native'] for c in ['scalar','branch','madd_short','madd_loop','detach_wait']}
        for arm,identity in m['arms'].items():
            assert identity['commit']==BASE
            for p,h in identity['sources'].items():
                assert digest(source(p,name=='ab' and arm=='B'))==h,(name,arm,p)
        if name=='aa': assert m['arms']['A']==m['arms']['B']
        manifests[name]=m; timing[name]=result
    assert manifests['ab']['arms']['A']==manifests['aa']['arms']['A']
    machine=load(OUT/'machine.json.gz'); sizes={}
    for arm,data in machine.items():
        assert data['binary_sha256']==manifests['ab']['arms'][arm]['binary_sha256']
        assert any('0x0000000100473e60' in s and '_bzero' in s for s in data['zero_imports'])
        assert len(data['sections'])==(1 if arm=='A' else 2)
        sizes[arm]={}
        for symbol,section in data['sections'].items():
            n=len(re.findall(r'^\s*[0-9a-f]+:',section['stdout'],re.M))
            assert section['exit_code']==0 and n==section['instructions']
            sizes[arm]['wrapper' if 'run_until_boundary' in symbol else 'dispatcher']=n*4
        wrapper=next(s['stdout'] for k,s in data['sections'].items() if 'run_until_boundary' in k)
        assert '#0x1000' in wrapper and '<dyld_stub_binder+0x100473e60>' in wrapper
        if arm=='B':
            assert '#0x11' in wrapper
            assert len(re.findall(r'stp\s+q0, q0',wrapper))==4
    layout=load(OUT/'layout.json.gz')
    assert all(r['exit_code']==0 for r in layout['records'])
    assert layout['overlay_sha256']==digest((ROOT/'evidence/ci-layout/diagnostic.patch').read_bytes())
    for p,h in layout['sources'].items(): assert digest(source(p,True))==h,p
    text='\n'.join(r['stdout'] for r in layout['records'])
    obj=json.loads(text.split('CI_LAYOUT ',1)[1].splitlines()[0])
    assert obj['unit']==120 and obj['state']==312 and obj['op']==56
    future=re.findall(r'CI_FUTURE (\w+) (\w+) (\d+)',text)
    assert len(future)==18 and all(int(n)==(928 if b=='tape' else 664) for _,b,n in future)
    assert len(re.findall('CI_EXECUTED .* both status=Ok stdout=exact',text))==9
    assert set(map(int,re.findall(r'CK slots=(\d+)',text)))=={0,1,2,3,7,15,31,127}
    checks=json.loads((OUT/'verification.json').read_text())
    assert len(checks)==7 and all(c['exit_code']==0 for c in checks)
    result=dict(base=BASE,timing=timing,machine_bytes=sizes,candidate_layout=obj,
                sync_stack_reserved_bytes={'A':4256,'B_wrapper_plus_dispatcher':4368},
                small_initialization_bytes={'A':4096,'B':128},
                future_bytes={'tape':928,'native':664},pmu_measured=False,rss_measured=False)
    (OUT/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
    aa={(c['backend'],c['case']):c for c in timing['aa']['cells']}
    for c in timing['ab']['cells']: print(c['backend'],c['case'],c['medians_ns'],round(c['b_over_a'],5),round(aa[c['backend'],c['case']]['b_over_a'],5))
    print('machine bytes',sizes,'candidate unit',obj['unit'])
if __name__=='__main__': main()
