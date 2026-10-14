"""Bounded S extension; serial WSL only. Original successful raw is untouched.
Usage: SCRIPT curve|reference|first OUTPUT. Three process rounds, CPU 2.
New curve cohort is separately named and never pooled with original observations.
"""
from collections import Counter
import gzip
import hashlib
import json
from pathlib import Path
import resource
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT.parent
N = [100, 300, 1000, 3000, 10000, 30000, 100000]
SHAPES = [('scalar', 0)] + [(c, n) for c in ['weighted', 'stateful'] for n in N]
ORIGINAL = {('scalar',0),('weighted',100),('stateful',100),('weighted',10000),('stateful',100000)}
PYPY = BASE / 'runtime/pypy3.11-v7.3.20-linux64/bin/pypy3'
NODE = '/home/na/.nvm/versions/node/v24.20.0/bin/node'
PYODIDE = BASE / 'runtime/node_modules/pyodide'
GUEST = Path('/home/na/projects/agent-python-runtime/dist/pysolate.wasm')
ENV = dict(MINIPY_COMMON='1', MINIPY_CHUNK='256')


def cpu():
    return sum(int((r.ru_utime + r.ru_stime) * 1e9) for r in
               [resource.getrusage(resource.RUSAGE_SELF), resource.getrusage(resource.RUSAGE_CHILDREN)])


def spec(key, argv, kind, expected, drop=0, **tags):
    return dict(key=key, argv=[str(x) for x in ['taskset', '-c', '2', *argv]],
                kind=kind, expected=expected, discard=drop, tags=tags)


def schedule(mode):
    work = []
    for ri in range(3):
        primary = ['native', 'interpreter', 'monty', 'reference']
        primary = primary[ri:] + primary[:ri]
        if ri % 2: primary.reverse()
        if mode == 'curve':
            for c, n in SHAPES:
                if (c,n) in ORIGINAL: continue  # preserve/reuse successful original cells
                for m in ['cold', 'warm']:
                    for e in primary:
                        work.append(spec(f'curve-{ri}-{e}-{c}-{n}-{m}', [BASE/'speed', e, c, m, n, 41], 'speed', 41, 11, round=ri, engine=e, case=c, rows=n, mode=m))
                arms = [('cpython-resident',sys.executable,'cpython-resident'), ('cpython-prepared',sys.executable,'cpython-prepared'), ('cpython-spawn',sys.executable,'cpython-spawn'), ('cpython-fork',sys.executable,'cpython-fork'), ('pypy-resident',PYPY,'cpython-resident'), ('pypy-prepared',PYPY,'cpython-prepared')]
                arms = arms[ri:] + arms[:ri]
                if ri % 2: arms.reverse()
                for label, exe, e in arms:
                    reps, drop = (241,201) if label.startswith('pypy') else (41,11)
                    cfg = dict(label=label,engine=e,case=c,rows=n,kind='speed',reps=reps)
                    work.append(spec(f'curve-{ri}-{label}-{c}-{n}', [exe,ROOT/'tools/s_wsl_python.py','speed',json.dumps(cfg)], 'speed', reps, drop, round=ri, engine=label, case=c, rows=n, mode='warm' if label.endswith('prepared') else 'cold'))
                for label in ['pyodide-resident','pyodide-prepared'][::(-1 if ri%2 else 1)]:
                    cfg = dict(engine=label,case=c,rows=n,kind='speed',reps=21)
                    work.append(spec(f'curve-{ri}-{label}-{c}-{n}', [NODE,ROOT/'tools/s_wsl_curve.mjs',PYODIDE,json.dumps(cfg)], 'speed', 21, 1, round=ri, engine=label, case=c, rows=n, mode='warm' if label.endswith('prepared') else 'cold'))
                for e in ['fresh','cow'][::(-1 if ri%2 else 1)]:
                    item = spec(f'curve-{ri}-pysolate-{e}-{c}-{n}', [BASE/'pysolate-bench',e,c,'cold',n,11,0,ROOT/'fixtures',GUEST], 'speed',11,1,round=ri,engine='pysolate-'+e,case=c,rows=n,mode='cold')
                    # Exact Go JSON map field order/compact encoding matches adapter.
                    payload = json.dumps([dict(quantity=i%9-2,unit_price_cents=i*37%1000) for i in range(n)],separators=(',',':'))
                    if len(payload.encode()) > 1048576:
                        item['unsupported'] = dict(reason='old Guest MAX_MESSAGE=1048576; generated compact response exceeds bound', response_bytes=len(payload.encode()), source='guest/runtime.c:12,31', observed=False)
                    work.append(item)
        elif mode == 'reference':
            for c, n in SHAPES[1:]:
                for m in ['kernel','full']:
                    for e in ['assembly','rust'][::(-1 if ri%2 else 1)]:
                        work.append(spec(f'reference-{ri}-{e}-{c}-{n}-{m}', [BASE/'assembly-extension',e,m,c,n,41,0], 'assembly-speed',41,11,round=ri,engine=e,case=c,rows=n,mode=m))
        elif mode == 'first':
            # Same fresh process supplies all phase timestamps. Three small/large
            # representative requests; old ABI bound is respected at N=10000.
            for c,n in [('scalar',0),('weighted',10000),('stateful',10000)]:
                for e in primary[:3]:
                    if e == 'reference': continue
                    work.append(spec(f'first-{ri}-{e}-{c}-{n}', [BASE/'speed-extension',e,c,'first-request',n,1], 'first-request',1,round=ri,engine=e,case=c,rows=n))
                # Every primary executable, independent of rotated primary list.
                existing = {i['tags']['engine'] for i in work if i['tags'].get('round')==ri and i['tags'].get('case')==c}
                for e in ['native','interpreter','monty']:
                    if e not in existing:
                        work.append(spec(f'first-{ri}-{e}-{c}-{n}', [BASE/'speed-extension',e,c,'first-request',n,1], 'first-request',1,round=ri,engine=e,case=c,rows=n))
                for label,exe in [('cpython',sys.executable),('pypy',PYPY)]:
                    work.append(spec(f'first-{ri}-{label}-{c}-{n}', [exe,ROOT/'tools/s_wsl_first.py',c,n], 'first-request',1,round=ri,engine=label,case=c,rows=n))
                work.append(spec(f'first-{ri}-pyodide-{c}-{n}', [NODE,ROOT/'tools/s_wsl_first.mjs',PYODIDE,c,n], 'first-request',1,round=ri,engine='pyodide-node',case=c,rows=n))
                for e in ['fresh','cow'][::(-1 if ri%2 else 1)]:
                    work.append(spec(f'first-{ri}-pysolate-{e}-{c}-{n}', [BASE/'pysolate-extension',e,c,'first-request',n,1,0,ROOT/'fixtures',GUEST], 'first-request',1,round=ri,engine='pysolate-'+e,case=c,rows=n))
        else: raise ValueError(mode)
    assert len({i['key'] for i in work}) == len(work)
    return work


def validate(rs,item):
    rows = [r for r in rs if r['kind']==item['kind']]
    assert len(rows)==item['expected'] and all(r['completed'] for r in rows), item['key']
    assert all((r['case'],r['rows'])==(item['tags']['case'],item['tags']['rows']) for r in rows)
    if item['kind'] != 'first-request':
        assert sorted(r['sample'] for r in rows)==list(range(item['expected']))
    else:
        for r in rows:
            assert sum(r[k] for k in ['process_startup_ns','runtime_init_ns','source_prepare_ns','execute_ns','reap_unassigned_ns'])==r['wall_ns']
            assert all(r[k]>=0 for k in ['process_startup_ns','runtime_init_ns','source_prepare_ns','execute_ns','reap_unassigned_ns'])
    return rows


def main(mode,out):
    out.mkdir(parents=True,exist_ok=True)
    work = schedule(mode)
    commands = out/(mode+'-commands.json')
    if commands.exists(): assert json.loads(commands.read_text())==work
    else: commands.write_text(json.dumps(work,indent=2)+'\n')
    files = [BASE/'speed',BASE/'speed-extension',BASE/'assembly-extension',BASE/'pysolate-bench',BASE/'pysolate-extension',GUEST,Path(NODE),PYPY,ROOT/'bench/src/weighted_x86_64.S',ROOT/'bench/src/assembly_reference.rs',ROOT/'bench/src/bin/assembly_ab.rs',ROOT/'bench/src/bin/interpreter_ab.rs',Path(__file__),ROOT/'tools/s_wsl_first.py',ROOT/'tools/s_wsl_first.mjs',ROOT/'tools/s_wsl_curve.mjs',ROOT/'tools/s_wsl_python.py',ROOT/'tools/final_python.py',ROOT/'tools/s_wsl_pysolate.go']
    identity = out/(mode+'-identity.json')
    if not identity.exists():
        identity.write_text(json.dumps(dict(files={str(p):dict(sha256=hashlib.sha256(p.read_bytes()).hexdigest(),bytes=p.stat().st_size) for p in files},
            preflight=subprocess.check_output(['bash','-lc','uname -a; uptime; free -h; ps -eo pid,comm,pcpu,pmem --sort=-pcpu'],text=True),
            units='ns',rounds=3,cpu='controller SELF + reaped CHILDREN for fresh first-request; executable SELF for Rust and Node; Python SELF + children for isolated request modes'),indent=2)+'\n')
    status = []
    for ix,item in enumerate(work):
        raw=out/(item['key']+'.jsonl.gz')
        if 'unsupported' in item:
            status.append(dict(key=item['key'],state='unsupported',scheduled_rows=item['expected'],**item['unsupported']))
        else:
            try:
                if raw.exists():
                    with gzip.open(raw,'rt') as f:rs=[json.loads(l) for l in f]
                else:
                    start,cp=time.perf_counter_ns(),cpu()
                    text=subprocess.check_output(item['argv'],cwd=ROOT,env={**__import__('os').environ,**ENV},text=True,stderr=subprocess.PIPE,timeout=600)
                    end,cost=time.perf_counter_ns(),cpu()-cp
                    rs=[json.loads(l) for l in text.splitlines()]
                    if mode=='first':
                        for r in rs:
                            if r['kind']!='first-request':continue
                            a,b,c,d=[r[k] for k in ['init_begin_ns','ready_ns','prepared_ns','executed_ns']]
                            assert start<=a<=b<=c<=d<=end, (start,a,b,c,d,end)
                            r.update(wall_ns=end-start,cpu_ns=cost,process_startup_ns=a-start,runtime_init_ns=b-a,
                                     source_prepare_ns=c-b,execute_ns=d-c,reap_unassigned_ns=end-d,
                                     launch_ns=start,reaped_ns=end,boundary='fresh process launch through response and reap')
                    for r in rs:r.update(**item['tags'],discard=item['discard'],command_key=item['key'])
                    validate(rs,item)
                    with gzip.open(raw,'wt') as f:
                        for r in rs:f.write(json.dumps(r)+'\n')
                validate(rs,item)
                status.append(dict(key=item['key'],state='completed',rows=item['expected']))
            except Exception as exc:
                error=repr(exc)+(('\n'+exc.stderr) if isinstance(exc,subprocess.CalledProcessError) else '')
                status.append(dict(key=item['key'],state='failed',scheduled_rows=item['expected'],error=error))
        (out/(mode+'-status.json')).write_text(json.dumps(status,indent=2)+'\n')
        if ix%10==0:print(mode,ix,len(work),item['key'],status[-1]['state'],flush=True)
    print(mode,dict(Counter(s['state'] for s in status)),flush=True)


if __name__=='__main__':main(sys.argv[1],Path(sys.argv[2]).resolve())
