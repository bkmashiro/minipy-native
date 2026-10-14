"""Run inside idle Linux VM after addon campaign; bounded noise + lifecycle probes.
argv: OUT_DIR NODE_EXEC NODE_RUNTIME. LD_LIBRARY_PATH must be task-local.
"""
import gzip
import hashlib
import json
import os
from pathlib import Path
import resource
import subprocess
import sys
import time


def cpu():
    result=0
    for who in [resource.RUSAGE_SELF,resource.RUSAGE_CHILDREN]:
        r=resource.getrusage(who);result+=int((r.ru_utime+r.ru_stime)*1e9)
    return result


def main():
    out=Path(sys.argv[1]); node=sys.argv[2]; runtime=sys.argv[3]
    raw=out/'scalar-recheck.jsonl.gz';assert not raw.exists(),'preserve raw'
    count=0;commands=[]
    with gzip.open(raw,'wt') as f:
        for ri in range(3):
            engines=['native','interpreter','monty','reference'];order=engines[ri:]+engines[:ri]
            if ri%2:order.reverse()
            for e in order:
                cmd=['./speed',e,'scalar','warm','0','1001'];commands.append(cmd)
                rs=[json.loads(l) for l in subprocess.check_output(cmd,text=True).splitlines()]
                assert len(rs)==1001 and all(r['completed'] for r in rs)
                for r in rs:r['round']=ri;f.write(json.dumps(r)+'\n');count+=1
                f.flush()
    assert count==12012
    init=[]
    for ri in range(3):
        order=['cpython','pyodide'] if ri%2==0 else ['pyodide','cpython']
        for e in order:
            cmd=[sys.executable,'-I','final_python.py','worker'] if e=='cpython' else [node,'final_pyodide.mjs',runtime,str(out),'init',str(ri)]
            payload=json.dumps({'case':'scalar','rows':0})+'\n' if e=='cpython' else None
            c=cpu();t=time.perf_counter_ns()
            result=subprocess.run(cmd,input=payload,text=True,capture_output=True,check=True,timeout=30)
            wall=time.perf_counter_ns()-t;c=cpu()-c
            response=json.loads(result.stdout)
            if e=='cpython':assert response['stdout']=='734\nTrue\n'
            init.append({'kind':'process-initialization','engine':e,'round':ri,
                         'wall_ns':wall,'cpu_ns':c,'response':response,
                         'boundary':'fresh CPython/imports/scalar request/IPC/reap' if e=='cpython' else 'fresh Node/imports/WASM/stdlib/harness initialization/reap',
                         'version':sys.version if e=='cpython' else response['version']})
    (out/'process-initialization.jsonl').write_text(''.join(json.dumps(r)+'\n' for r in init))
    (out/'probe-identity.json').write_text(json.dumps({'commands':commands,'raw_count':count,'initialization_processes':6,
        'source_sha256':{p:hashlib.sha256(Path(p).read_bytes()).hexdigest() for p in ['final_python.py','final_pyodide.mjs','final_probes.py']},
        'original_speed_sha256':hashlib.sha256(Path('speed').read_bytes()).hexdigest(),
        'cpu':'whole in-VM controller SELF + reaped CHILDREN; direct wall including child teardown; no SSH latency'},indent=2)+'\n')
    print('probes complete',count,len(init),flush=True)


if __name__=='__main__':main()
