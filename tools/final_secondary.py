"""L serial secondary deployment, no historical measurements.
Usage: final_secondary.py HOST KEY KNOWN. Node installed task-locally first.
"""
import gzip
import hashlib
import json
from pathlib import Path
import shlex
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'bench/results/l-final'
REMOTE = '/root/minipy-native-l'


def main():
    host, key, known = sys.argv[1:]
    opts = ['-i',key,'-o',f'UserKnownHostsFile={known}','-o','StrictHostKeyChecking=yes','-o','BatchMode=yes']
    ssh = ['ssh',*opts,host]
    def run(argv, timeout=900):
        return subprocess.check_output(argv, text=True, stderr=subprocess.STDOUT, timeout=timeout)
    identity = {'commands': [], 'failures': [],
                'preflight': run(ssh + ['uptime; free -m; df -h /; ps']),
                'source_sha256': {p: hashlib.sha256((ROOT/p).read_bytes()).hexdigest()
                                  for p in ['tools/final_python.py','tools/final_pyodide.mjs','tools/final_secondary.py']}}
    node = f'LD_LIBRARY_PATH={REMOTE}/runtime/root/usr/lib:{REMOTE}/runtime/root/lib {REMOTE}/runtime/root/usr/bin/node'
    try:
        for name in ['final_python.py','final_pyodide.mjs']:
            run(['scp',*opts,str(ROOT/'tools'/name),f'{host}:{REMOTE}/{name}'])
            assert run(ssh + [f'sha256sum {REMOTE}/{name}']).split()[0] == identity['source_sha256']['tools/'+name]
        def command(cmd):
            identity['commands'].append(cmd)
            try: return run(ssh + [cmd])
            except Exception as exc:
                identity['failures'].append({'command':cmd,'error':str(exc)})
                raise
        identity['node_version'] = command(node + ' --version').strip()
        identity['cpython_version'] = command('python3 --version').strip()
        identity['runtime_files'] = command(f'cd {REMOTE} && sha256sum runtime/package/pyodide.asm.wasm runtime/package/python_stdlib.zip runtime/package/pyodide.mjs runtime/root/usr/bin/node')
        smoke = command(f'cd {REMOTE} && {node} final_pyodide.mjs runtime/package results init')
        (OUT/'pyodide-smoke.json').write_text(smoke)
        text = command(f'cd {REMOTE} && python3 final_python.py campaign results')
        (OUT/'cpython-campaign.log').write_text(text)
        print(text,flush=True)
        for ri in range(3):
            text = command(f'cd {REMOTE} && {node} final_pyodide.mjs runtime/package results round {ri}')
            print(text,flush=True)
        memory = []
        for ri in range(3):
            for code in ['unique','shared']:
                rows = [json.loads(l) for l in command(f'cd {REMOTE} && {node} final_pyodide.mjs runtime/package results memory {code}').splitlines()]
                assert [r['phase'] for r in rows] == ['startup','load_wait','record_wait','reclaimed']
                for r in rows: r['round'] = ri; memory.append(r)
        (OUT/'pyodide-memory.jsonl').write_text(''.join(json.dumps(r)+'\n' for r in memory))
        for name in ['cpython.jsonl.gz','cpython-identity.json','cpython-memory.jsonl',
                     'pyodide.jsonl.gz','pyodide-initialization.jsonl',
                     'pyodide-identity-0.json','pyodide-identity-1.json','pyodide-identity-2.json']:
            run(['scp',*opts,f'{host}:{REMOTE}/results/{name}',str(OUT/name)])
        for stem, expected in [('cpython',10860),('pyodide',5430)]:
            with gzip.open(OUT/(stem+'.jsonl.gz'),'rt') as f: rows = [json.loads(l) for l in f]
            assert len(rows)==expected and all(r['completed'] for r in rows)
        identity['verified_counts'] = {'cpython':10860,'pyodide':5430,'cpython_memory':48,'pyodide_memory':24}
    finally:
        identity['postflight'] = run(ssh + ['uptime; free -m; df -h /; ps'])
        (OUT/'secondary-identity.json').write_text(json.dumps(identity,indent=2)+'\n')


if __name__ == '__main__': main()
