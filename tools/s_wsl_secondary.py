"""S serial supplementary Pyodide, startup and prepared-owner diagnostics.
Usage on WSL: python3 tools/s_wsl_secondary.py OUTPUT
Run only after main campaign, never alongside a build or another experiment.
"""
import gzip
import hashlib
import json
from pathlib import Path
import resource
import subprocess
import sys
import time

ROOT=Path(__file__).resolve().parents[1]
BASE=ROOT.parent
OUT=Path(sys.argv[1]).resolve()
NODE='/home/na/.nvm/versions/node/v24.20.0/bin/node'
PYODIDE=BASE/'runtime/node_modules/pyodide'
PYPY=BASE/'runtime/pypy3.11-v7.3.20-linux64/bin/pypy3'


def cpu():
    return sum(int((r.ru_utime+r.ru_stime)*1e9) for r in [resource.getrusage(resource.RUSAGE_SELF),resource.getrusage(resource.RUSAGE_CHILDREN)])


def run(argv,stdin=None):
    return subprocess.check_output([str(a) for a in ['taskset','-c','2',*argv]],cwd=ROOT,input=stdin,text=True,stderr=subprocess.PIPE,timeout=600)


def main():
    identity=OUT/'secondary-identity.json'
    if identity.exists():
        assert len(json.loads(identity.read_text())['commands'])==126
        print('secondary already complete; reduction validates all retained rows',flush=True)
        return
    bodies={c:(ROOT/f'fixtures/common_{c}.py').read_text() for c in ['scalar','weighted','stateful']}
    (ROOT/'common-bodies.json').write_text(json.dumps(bodies)+'\n')
    commands=[]
    for ri in range(3):
        argv=[NODE,ROOT/'tools/final_pyodide.mjs',PYODIDE,OUT,'common',ri]
        if not (OUT/f'common-pyodide-identity-{ri}.json').exists(): print(run(argv),flush=True)
        commands.append(dict(argv=[str(a) for a in argv],kind='pyodide-round',expected_rows=1810))
    path=OUT/'pyodide-memory.jsonl'
    if not path.exists():
        rows=[]
        for ri in range(3):
            for code in ['unique','shared']:
                argv=[NODE,ROOT/'tools/final_pyodide.mjs',PYODIDE,OUT,'memory',code]
                batch=[json.loads(l) for l in run(argv).splitlines()]
                assert [r['phase'] for r in batch]==['startup','load_wait','record_wait','reclaimed']
                for r in batch:r.update(round=ri,corpus='tuple-secondary')
                rows.extend(batch)
                commands.append(dict(argv=[str(a) for a in argv],kind='pyodide-memory',expected_rows=4))
        path.write_text(''.join(json.dumps(r)+'\n' for r in rows))
    prepared=OUT/'prepared-owner.jsonl'
    if not prepared.exists():
        rows=[]
        for ri in range(3):
            order=['native','interpreter','monty'] if ri%2==0 else ['monty','interpreter','native']
            for case in ['scalar','weighted','stateful']:
                for count in [1,32]:
                    for code in ['unique','shared']:
                        for engine in order:
                            argv=[BASE/'prepared',engine,case,count,code]
                            batch=[json.loads(l) for l in run(argv).splitlines()]
                            assert [r['phase'] for r in batch]==['startup','retained','reclaimed']
                            for r in batch:r['round']=ri
                            rows.extend(batch)
                            commands.append(dict(argv=[str(a) for a in argv],kind='prepared-owner',expected_rows=3))
        assert len(rows)==324
        prepared.write_text(''.join(json.dumps(r)+'\n' for r in rows))
    startup=OUT/'process-startup.jsonl'
    if not startup.exists():
        rows=[]
        for ri in range(3):
            order=[('cpython',sys.executable),('pypy',PYPY),('pyodide',NODE)]
            if ri%2:order.reverse()
            for engine,executable in order:
                argv=[executable,ROOT/'tools/final_python.py','worker'] if engine!='pyodide' else [NODE,ROOT/'tools/final_pyodide.mjs',PYODIDE,OUT,'init',ri]
                payload=json.dumps(dict(case='scalar',rows=0,body=bodies['scalar']))+'\n' if engine!='pyodide' else None
                start,cp=time.perf_counter_ns(),cpu()
                text=run(argv,payload)
                elapsed,cost=time.perf_counter_ns()-start,cpu()-cp
                response=json.loads(text)
                if engine!='pyodide':assert response['stdout']=='734\nTrue\n'
                rows.append(dict(kind='process-startup',engine=engine,round=ri,wall_ns=elapsed,cpu_ns=cost,response=response,
                                 boundary='fresh process, imports, scalar common request, IPC/reap' if engine!='pyodide' else 'fresh Node process, Wasm compile/instantiate, stdlib/harness imports and reap'))
                commands.append(dict(argv=[str(a) for a in argv],kind='process-startup',expected_rows=1))
        startup.write_text(''.join(json.dumps(r)+'\n' for r in rows))
    files=list(PYODIDE.glob('*'))+[BASE/'prepared',ROOT/'tools/s_wsl_secondary.py',BASE/'runtime/package-lock.json',Path('/home/na/projects/agent-python-runtime/dist/pysolate.manifest.json')]
    (OUT/'secondary-identity.json').write_text(json.dumps(dict(commands=commands,files={str(f):dict(sha256=hashlib.sha256(f.read_bytes()).hexdigest(),bytes=f.stat().st_size) for f in files if f.is_file()},
            postflight=run(['bash','-lc','uptime; free -h; ps -eo pid,comm,pcpu,pmem --sort=-pcpu; git -C /home/na/projects/agent-python-runtime status --short']),
            cpu='startup uses controller SELF + reaped CHILDREN; no double counted child self report'),indent=2)+'\n')
    print('secondary complete',flush=True)


if __name__=='__main__':main()
