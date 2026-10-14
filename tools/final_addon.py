"""L incremental common-body + genuine assembly measurements, serial Linux.
Usage: final_addon.py HOST KEY KNOWN COMMON_SPEED ASM_SPEED ASM_MEMORY ASM_TEST
"""
import gzip
import hashlib
import json
from pathlib import Path
import shlex
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'bench/results/l-final'
REMOTE='/root/minipy-native-l'
PLAN=json.loads((OUT/'addon-plan.json').read_text())


def main():
    host,key,known,common,assembly,memory,test=sys.argv[1:]
    opts=['-i',key,'-o',f'UserKnownHostsFile={known}','-o','StrictHostKeyChecking=yes','-o','BatchMode=yes']
    ssh=['ssh',*opts,host]
    def run(argv): return subprocess.check_output(argv,text=True,stderr=subprocess.STDOUT,timeout=300)
    sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
    identity={'runtime_commit':run(['git','rev-parse','HEAD']).strip(),'commands':[],
              'preflight':run(ssh+['uptime; free -m; df -h /; ps']),
              'binaries':{},'failures':[],
              'source_sha256':{str(p.relative_to(ROOT)):sha(p) for pattern in ['src/*.rs','bench/src/*.rs','bench/src/*.S','bench/src/bin/*.rs','fixtures/common_*.py','tools/final_*.py','tools/final_*.mjs','bench/results/l-final/addon-plan.json'] for p in ROOT.glob(pattern)}}
    def command(cmd):
        identity['commands'].append(cmd)
        try:return run(ssh+[cmd])
        except Exception as exc:
            identity['failures'].append({'command':cmd,'error':str(exc)})
            raise
    assert identity['runtime_commit']==PLAN['runtime_commit']
    try:
        for name,p in [('common',common),('assembly',assembly),('assembly-memory',memory),('assembly-test',test)]:
            run(['scp',*opts,p,f'{host}:{REMOTE}/{name}'])
            assert run(ssh+[f'sha256sum {REMOTE}/{name}']).split()[0]==sha(p)
            identity['binaries'][name]={'sha256':sha(p),'bytes':Path(p).stat().st_size}
        command(f'mkdir -p {REMOTE}/fixtures')
        for name in ['weighted_scan.py','stateful_branch.py']:
            run(['scp',*opts,str(ROOT/'fixtures'/name),f'{host}:{REMOTE}/fixtures/{name}'])
        (OUT/'assembly-tests.log').write_text(command(f'MINIPY_FIXTURES={REMOTE}/fixtures {REMOTE}/assembly-test --test-threads=1'))
        # Real common source smoke is the same byte body in all three primary engines.
        with (OUT/'addon-smoke.jsonl').open('w') as f:
            for engine in ['native','interpreter','monty']:
                f.write(command(f'MINIPY_COMMON=1 {REMOTE}/common {engine} weighted cold 100 2'))
            for arm in ['assembly','rust']:
                f.write(command(f'{REMOTE}/assembly {arm} full stateful 100 2 1000'))
        bodies={case:(ROOT/f'fixtures/common_{case}.py').read_text() for case in ['scalar','weighted','stateful']}
        (OUT/'common-bodies.json').write_text(json.dumps(bodies,indent=2)+'\n')
        for p in [ROOT/'tools/final_python.py',ROOT/'tools/final_pyodide.mjs',OUT/'common-bodies.json']:
            run(['scp',*opts,str(p),f'{host}:{REMOTE}/{p.name}'])
            assert run(ssh+[f'sha256sum {REMOTE}/{p.name}']).split()[0]==sha(p)
        count=0
        raw=OUT/'common-primary.jsonl.gz'; assert not raw.exists(),'preserve raw'
        with gzip.open(raw,'wt') as f:
            def collect(engine,case,mode,n,reps,ri,delay=0):
                nonlocal count
                cmd=f'MINIPY_COMMON=1 MINIPY_TOOL_DELAY_US={delay} MINIPY_CHUNK=256 '+shlex.join([f'{REMOTE}/common',engine,case,mode,str(n),str(reps)])
                rows=[json.loads(l) for l in command(cmd).splitlines()]
                assert len(rows)==reps*(40 if mode.startswith('mixed-') else 1)
                assert all(r['completed'] for r in rows)
                for r in rows:r.update(round=ri,corpus='common');f.write(json.dumps(r)+'\n')
                f.flush();count+=len(rows)
            for ri in range(3):
                engines=PLAN['common']['primary_engines'];order=engines[ri:]+engines[:ri]
                if ri%2:order.reverse()
                for case,n in PLAN['common']['shapes']:
                    for mode in ['cold','warm']:
                        for e in order:collect(e,case,mode,n,41,ri)
                for mix in ['light','scan']:
                    for mode,delay in PLAN['common']['mixed_modes']:
                        for e in order:collect(e,mix,mode,0,5,ri,delay)
                print('common primary round',ri,'complete',count,flush=True)
        assert count==14490
        text=command(f'cd {REMOTE} && python3 final_python.py common-campaign results common-bodies.json')
        (OUT/'common-cpython.log').write_text(text);print(text,flush=True)
        node=f'LD_LIBRARY_PATH={REMOTE}/runtime/root/usr/lib:{REMOTE}/runtime/root/lib {REMOTE}/runtime/root/usr/bin/node'
        for ri in range(3):print(command(f'cd {REMOTE} && {node} final_pyodide.mjs runtime/package results common {ri}'),flush=True)
        for name in ['common-cpython.jsonl.gz','common-cpython-identity.json','common-pyodide.jsonl.gz',
                     'common-pyodide-initialization.jsonl','common-pyodide-identity-0.json','common-pyodide-identity-1.json','common-pyodide-identity-2.json']:
            run(['scp',*opts,f'{host}:{REMOTE}/results/{name}',str(OUT/name)])
        count=0
        raw=OUT/'assembly.jsonl.gz';assert not raw.exists(),'preserve raw'
        with gzip.open(raw,'wt') as f:
            for ri in range(3):
                order=['assembly','rust'] if ri%2==0 else ['rust','assembly']
                for case in ['weighted','stateful']:
                    for mode in ['kernel','full']:
                        for n in (PLAN['assembly']['kernel_rows'] if mode=='kernel' else PLAN['assembly']['full_rows']):
                            for delay in ([0] if mode=='kernel' else [0,1000]):
                                for e in order:
                                    cmd=shlex.join([f'{REMOTE}/assembly',e,mode,case,str(n),'101',str(delay)])
                                    rows=[json.loads(l) for l in command(cmd).splitlines()]
                                    assert len(rows)==101 and all(r['completed'] for r in rows)
                                    for r in rows:r.update(round=ri,delay_us=delay);f.write(json.dumps(r)+'\n')
                                    count+=len(rows);f.flush()
                print('assembly round',ri,'complete',count,flush=True)
        assert count==12120
        rows=[]
        for ri in range(3):
            for tasks in [1,8,32]:
                batch=[json.loads(l) for l in command(f'{REMOTE}/assembly-memory assembly hold weighted 10000 {tasks} 0').splitlines()]
                assert [r['phase'] for r in batch]==['startup','load_wait','record_wait','reclaimed']
                for r in batch:r['round']=ri;rows.append(r)
        assert len(rows)==36
        (OUT/'assembly-memory.jsonl').write_text(''.join(json.dumps(r)+'\n' for r in rows))
        for stem,expected in [('common-primary',14490),('common-cpython',5430),('common-pyodide',5430),('assembly',12120)]:
            with gzip.open(OUT/(stem+'.jsonl.gz'),'rt') as f:raw=[json.loads(l) for l in f]
            assert len(raw)==expected and all(r['completed'] for r in raw)
        identity['verified_counts']={'common_primary':14490,'common_cpython':5430,'common_pyodide':5430,'assembly':12120,'assembly_memory':36}
    finally:
        identity['postflight']=run(ssh+['uptime; free -m; df -h /; ps'])
        (OUT/'addon-identity.json').write_text(json.dumps(identity,indent=2)+'\n')


if __name__=='__main__':main()
