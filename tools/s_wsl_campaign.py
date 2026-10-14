"""S serial WSL campaign, run on WSL from task-local frozen source.
Usage: python3 tools/s_wsl_campaign.py PLAN OUTPUT
Raw and targets stay on WSL. Resume skips only validated completed commands.
"""
import gzip
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
P = json.loads(Path(sys.argv[1]).read_text())
OUT = Path(sys.argv[2]).resolve()
OUT.mkdir(parents=True, exist_ok=True)
BASE = ROOT.parent
NODE = '/home/na/.nvm/versions/node/v24.20.0/bin/node'
PYPY = BASE / 'runtime/pypy3.11-v7.3.20-linux64/bin/pypy3'
SPEED = BASE / 'speed'
MEMORY = BASE / 'memory'
OLD = BASE / 'pysolate-bench'
GUEST = Path('/home/na/projects/agent-python-runtime/dist/pysolate.wasm')


def command(argv, env=None, timeout=600):
    return subprocess.check_output([str(x) for x in argv], cwd=ROOT,
                                   env={**os.environ, **(env or {})}, text=True,
                                   stderr=subprocess.PIPE, timeout=timeout)


def spec(key, argv, kind, expected, discard=0, env=None, **tags):
    return dict(key=key, argv=[str(a) for a in argv], kind=kind, expected=expected,
                discard=discard, env=env or {}, tags=tags)


def schedule():
    work = []
    env = dict(MINIPY_COMMON='1', MINIPY_CHUNK='256')
    for ri in range(3):
        order = P['primary_engines'][ri:] + P['primary_engines'][:ri]
        if ri % 2: order.reverse()
        for c, n in P['shapes']:
            for m in P['modes']:
                for e in order:
                    work.append(spec(f'primary-{ri}-{e}-{c}-{n}-{m}', ['taskset','-c','2',SPEED,e,c,m,n,41], 'speed',41,11,env,round=ri))
        for mix in ['light','scan']:
            for m in ['mixed-cold','mixed-warm']:
                for d in [0,1000]:
                    for e in order:
                        work.append(spec(f'primary-{ri}-{e}-{mix}-{m}-{d}', ['taskset','-c','2',SPEED,e,mix,m,0,5], 'mixed',200,1,{**env,'MINIPY_TOOL_DELAY_US':str(d),'MINIPY_INFLIGHT':'40'},round=ri))
    arms = [('cpython-resident',sys.executable,'cpython-resident'),('cpython-prepared',sys.executable,'cpython-prepared'),('cpython-spawn',sys.executable,'cpython-spawn'),('cpython-fork',sys.executable,'cpython-fork'),('pypy-resident',PYPY,'cpython-resident'),('pypy-prepared',PYPY,'cpython-prepared')]
    for ri in range(3):
        order = arms[ri:] + arms[:ri]
        if ri % 2: order.reverse()
        for c,n in P['shapes']:
            for label,python,engine in order:
                reps,drop=(241,201) if label.startswith('pypy') else (41,11)
                cfg=dict(label=label,engine=engine,case=c,rows=n,kind='speed',reps=reps)
                work.append(spec(f'python-{ri}-{label}-{c}-{n}', ['taskset','-c','2',python,ROOT/'tools/s_wsl_python.py','speed',json.dumps(cfg)],'speed',reps,drop,round=ri))
        for mix in ['light','scan']:
            for d in [0,1000]:
                for label,python,engine in order:
                    cfg=dict(label=label,engine=engine,mix=mix,kind='mixed',delay_us=d,inflight=40,reps=5)
                    work.append(spec(f'python-{ri}-{label}-{mix}-{d}', ['taskset','-c','2',python,ROOT/'tools/s_wsl_python.py','speed',json.dumps(cfg)],'mixed',200,1,round=ri))
    for ri in range(3):
        for code in ['unique','shared']:
            for tasks in [1,8,32]:
                for e in P['primary_engines']:
                    work.append(spec(f'memory-{ri}-{e}-{code}-{tasks}', ['taskset','-c','2',MEMORY,e,'weighted','hold',10000,tasks], 'hold',4,env={**env,'MINIPY_HOLD_CODE':code},round=ri))
                for python in [sys.executable,PYPY]:
                    label = 'pypy' if python == PYPY else 'cpython'
                    work.append(spec(f'memory-{ri}-{label}-{code}-{tasks}', ['taskset','-c','2',python,ROOT/'tools/s_wsl_python.py','memory',code,tasks], 'hold',4,round=ri))
        for e in ['cpython-spawn','cpython-fork']:
            work.append(spec(f'tree-{ri}-{e}', ['taskset','-c','2',sys.executable,ROOT/'tools/final_python.py','tree-memory',e],'secondary-hold',4,round=ri,representation='tuple-secondary'))
    for ri in range(3):
        order=['fresh','cow'] if ri % 2 == 0 else ['cow','fresh']
        for c,n in P['shapes']:
            for e in order:
                work.append(spec(f'old-{ri}-{e}-{c}-{n}', ['taskset','-c','2',OLD,e,c,'cold',n,11,0,ROOT/'fixtures',GUEST],'speed',11,1,round=ri))
        for mix in ['light','scan']:
            for d in [0,1000]:
                for e in order:
                    work.append(spec(f'old-{ri}-{e}-{mix}-{d}', ['taskset','-c','2',OLD,e,mix,'mixed-cold',0,5,d,ROOT/'fixtures',GUEST],'mixed',200,1,round=ri))
        for tasks in [1,8,32]:
            for e in order:
                work.append(spec(f'old-memory-{ri}-{e}-{tasks}', ['taskset','-c','2',OLD,e,'weighted','hold',10000,tasks,0,ROOT/'fixtures',GUEST],'hold',4,round=ri))
    return work


def validate(rows, item):
    observed=[r for r in rows if r['kind']==item['kind']]
    assert len(observed)==item['expected'], (item['key'],len(observed))
    if item['kind'] in ['speed','mixed']:
        assert all(r['completed'] for r in observed)
        samples=item['expected']//(40 if item['kind']=='mixed' else 1)
        assert sorted({r['sample'] for r in observed})==list(range(samples))
    else:
        assert [r['phase'] for r in observed]==P['memory']['phases']
    if item['kind']=='mixed':
        from collections import Counter
        for sample in range(item['expected']//40):
            rs=[r for r in observed if r['sample']==sample]
            assert len({(r['batch_wall_ns'],r['batch_cpu_ns']) for r in rs})==1
            assert Counter((r['case'],r['rows']) for r in rs)==Counter({tuple(s):w for s,w in zip(P['shapes'],P['mixed'][rs[0]['mix']+'_weights'])})


def old_abi_case(item):
    return item['key'].startswith('old-') and (item['kind']=='mixed' or (item['kind']=='speed' and item['argv'][7]=='100000'))


def main():
    work=schedule()
    assert len({i['key'] for i in work})==len(work)
    path=OUT/'commands.json'
    if path.exists(): assert json.loads(path.read_text())==work
    else: path.write_text(json.dumps(work,indent=2)+'\n')
    identity=OUT/'identity.json'
    if not identity.exists():
        files=[ROOT/p for p in ['src/interpreter.rs','bench/src/bin/interpreter_ab.rs','bench/Cargo.lock','tools/final_python.py','tools/final_pyodide.mjs','tools/s_wsl_python.py','tools/s_wsl_campaign.py','tools/s_wsl_pysolate.go']]
        files += [SPEED,MEMORY,OLD,GUEST,PYPY,Path(NODE)] + list((ROOT/'fixtures').glob('common_*.py'))
        identity.write_text(json.dumps(dict(runtime_commit=P['runtime_commit'],platform=platform.platform(),python=sys.version,
            rust=command(['rustc','+1.96.0','--version']),pypy=command([PYPY,'--version']),node=command([NODE,'--version']),
            preflight=command(['bash','-lc','uname -a; uptime; free -h; df -h /; lscpu; ps -eo pid,comm,pcpu,pmem --sort=-pcpu']),
            files={str(f):dict(sha256=hashlib.sha256(f.read_bytes()).hexdigest(),bytes=f.stat().st_size) for f in files},
            old_commit=command(['git','-C','/home/na/projects/agent-python-runtime','rev-parse','HEAD']),
            old_status=command(['git','-C','/home/na/projects/agent-python-runtime','status','--short']),
            scheduled_commands=len(work), scheduled_rows=sum(i['expected'] for i in work)),indent=2)+'\n')
    status=[]
    previous={s['key']:s for s in json.loads((OUT/'status.json').read_text())} if (OUT/'status.json').exists() else {}
    abi_path=ROOT/'bench/results/s-wsl/abi-limit.json'
    abi=json.loads(abi_path.read_text()) if abi_path.exists() else None
    if abi:
        assert hashlib.sha256(GUEST.read_bytes()).hexdigest()==abi['guest_sha256']
        (OUT/'runner-correction.json').write_text(json.dumps(dict(reason='Classify observed old Guest ABI rejection; do not repeat remaining unsupported cells or alter payload/ABI. Existing successful raw preserved.',
            original_runner_sha256=json.loads(identity.read_text())['files'][str(ROOT/'tools/s_wsl_campaign.py')]['sha256'],
            corrected_runner_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),abi=abi),indent=2)+'\n')
    # Real smoke every request engine before formal work. Immutable command files
    # distinguish smoke from retained warmup and measured observations.
    smoke=[]
    for item in work:
        if (OUT/'smoke.json').exists(): break
        if item['kind']=='speed' and item['key'].startswith(('primary-0','python-0','old-0')) and (item['key'].endswith('-weighted-100-cold') or item['key'].endswith('-weighted-100')):
            rows=[json.loads(l) for l in command(item['argv'],item['env']).splitlines()]
            validate(rows,item)
            smoke.extend([{**r,'smoke_key':item['key']} for r in rows if r['kind']=='speed'][:2])
    if smoke: (OUT/'smoke.json').write_text(json.dumps(smoke,indent=2)+'\n')
    for index,item in enumerate(work):
        if abi and old_abi_case(item):
            prior=previous.get(item['key'],{})
            status.append(dict(key=item['key'],state='unsupported',rows=0,scheduled_rows=item['expected'],reason=abi['reason'],
                               observed_failure=prior.get('observed_failure',prior.get('state')=='failed' and abi['reason'] in prior.get('error','')),
                               prior_error=prior.get('error',prior.get('prior_error'))))
            (OUT/'status.json').write_text(json.dumps(status,indent=2)+'\n')
            continue
        raw=OUT/(item['key']+'.jsonl.gz')
        try:
            if raw.exists():
                with gzip.open(raw,'rt') as f: rows=[json.loads(l) for l in f]
            else:
                text=command(item['argv'],item['env'])
                rows=[json.loads(l) for l in text.splitlines()]
                validate(rows,item)
                for r in rows: r.update(**item['tags'],discard=item['discard'],command_key=item['key'])
                with gzip.open(raw,'wt') as f:
                    for r in rows: f.write(json.dumps(r)+'\n')
            validate(rows,item)
            status.append(dict(key=item['key'],state='completed',rows=item['expected']))
        except (Exception,) as exc:
            error=repr(exc)
            if isinstance(exc,subprocess.CalledProcessError): error += '\n'+exc.stderr
            status.append(dict(key=item['key'],state='failed',error=error))
        (OUT/'status.json').write_text(json.dumps(status,indent=2)+'\n')
        if index%10==0: print(index,item['key'],status[-1]['state'],flush=True)
    print('scheduled',len(work),'completed',sum(s['state']=='completed' for s in status),'failed',sum(s['state']=='failed' for s in status),'unsupported',sum(s['state']=='unsupported' for s in status),flush=True)


if __name__=='__main__': main()
