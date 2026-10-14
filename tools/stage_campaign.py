"""I bounded campaign. Fixed reference is execution-only, not source compilation.
Usage: stage_campaign.py HOST KEY KNOWN_HOSTS [base|native-candidate]
Run serially. Uses existing H shapes/arrivals/windows; no background guest work.
"""
import gzip
import hashlib
import json
from pathlib import Path
import shlex
import subprocess
import sys
ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'bench/results/i-stage'
SHAPES = [('scalar',0),('weighted',100),('stateful',100),('weighted',10000),('stateful',100000)]
def run(args):
    return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT)
def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
    host,key,known,*args = sys.argv[1:]
    label = args[0] if args else 'base'
    opts = ['-i',key,'-o',f'UserKnownHostsFile={known}','-o','StrictHostKeyChecking=yes','-o','BatchMode=yes']
    ssh=['ssh',*opts,host]; scp=['scp',*opts]; remote='/root/minipy-native-i-stage'
    OUT.mkdir(parents=True,exist_ok=True)
    raw=OUT/f'{label}.jsonl.gz'
    assert not raw.exists(), 'preserve raw'
    assert label in ['base', 'native-candidate', 'interpreter-candidate']
    engine = 'interpreter' if label == 'interpreter-candidate' else 'native'
    configurations = [('base',e) for e in ['native','interpreter','reference','monty']] if label=='base' else [('base',engine),(label,engine)]
    identity={'head':run(['git','rev-parse','HEAD']).strip(),'patch':run(['git','diff','--binary']), 'source_sha256':{},'binaries':{},'commands':[], 'rust':run(['rustc','+1.96.0','--version']).strip(),'zig':run(['zig','version']).strip(),'vm_initial':'off; existing 2 CPU / 2048 MiB; started exclusively','preflight':run(ssh+['uname -a; uptime; free -m; df -h /; ps']), 'label':label, 'window':'H: mixed 21 batches keep >=3; shapes 101 requests keep >=21; 3 alternating rounds; 40 arrivals/100ms; 500us burst spacing; finite draining windows'}
    for pattern in ['src/*.rs','fixtures/*.py','tests/fixture_reference.rs','bench/src/bin/interpreter_ab.rs','bench/src/v2_common.rs','tools/stage_campaign.py','Cargo.toml','Cargo.lock','bench/Cargo.toml','bench/Cargo.lock']:
        for p in ROOT.glob(pattern): identity['source_sha256'][str(p.relative_to(ROOT))]=sha(p)
    run(ssh+[f'mkdir -p {remote}'])
    for build in sorted({b for b,_ in configurations}):
        for mode in ['speed','memory']:
            p=ROOT/f'target/i-{build}-{mode}-linux'
            if label!='base' and mode=='memory': continue
            run(scp+[str(p),f'{host}:{remote}/{build}-{mode}'])
            assert run(ssh+[f'sha256sum {remote}/{build}-{mode}']).split()[0]==sha(p)
            identity['binaries'][f'{build}-{mode}']={'sha256':sha(p),'bytes':p.stat().st_size}
    count=0
    with gzip.open(raw,'wt') as f:
        def collect(build,engine,case,mode,n,reps,round_id,delay=0):
            nonlocal count
            cmd=f'MINIPY_TOOL_DELAY_US={delay} '+shlex.join([f'{remote}/{build}-speed',engine,case,mode,str(n),str(reps)])
            identity['commands'].append(cmd)
            rows=[json.loads(l) for l in run(ssh+[cmd]).splitlines()]
            assert len(rows)==reps*(40 if mode.startswith('mixed-') else 1)
            assert all(r['completed'] for r in rows)
            for r in rows:
                r.update(build=build,round=round_id)
                f.write(json.dumps(r,separators=(',',':'))+'\n')
            count+=len(rows); f.flush()
        for ri in range(3):
            order=configurations[ri%len(configurations):]+configurations[:ri%len(configurations)]
            if ri==1: order.reverse()
            for mix in ['light','scan']:
                for mode,delay in [('mixed-cold',0),('mixed-cold',1000),('mixed-warm',0)]:
                    for build,engine in order: collect(build,engine,mix,mode,0,21,ri,delay)
            for case,n in SHAPES:
                for mode in ['cold','warm']:
                    for build,engine in order: collect(build,engine,case,mode,n,101,ri)
            print(label,'round',ri,'complete',flush=True)
    assert count==3*len(configurations)*(6*21*40+10*101)
    if label=='base':
        with (OUT/'memory.jsonl').open('w') as f:
            for mix in ['light','scan']:
                for engine in ['native','interpreter','reference','monty','monty-clear']:
                    cmd=shlex.join([f'{remote}/base-memory',engine,mix,'mixed-hold','0','1'])
                    identity['commands'].append(cmd)
                    rows=[json.loads(l) for l in run(ssh+[cmd]).splitlines()]
                    assert [r['phase'] for r in rows]==['startup','load_wait','record_wait','reclaimed']
                    for r in rows: f.write(json.dumps(r,separators=(',',':'))+'\n')
    identity['raw_count']=count
    identity['postflight']=run(ssh+['uptime; free -m; df -h /; ps'])
    (OUT/f'{label}-identity.json').write_text(json.dumps(identity,indent=2)+'\n')
if __name__=='__main__': main()
