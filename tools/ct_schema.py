#!/usr/bin/env python3
"""CT fixed matrix. All build/test/timing commands serialize on the project lock."""
import fcntl
import gzip
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
import threading
import time
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SCRATCH = Path.home() / '.hermes/cache/scratch'
OUT = ROOT / 'evidence/ct-schema'
CASES = [f'w{w}_{c}_{l}' for w in (2, 64) for c in (1, 4) for l in (1, 2)] + ['scalar', 'numeric', 'string', 'path_error']
TOP = ['top', '-l', '0', '-s', '1', '-n', '8', '-o', 'cpu', '-stats', 'pid,command,cpu,time,threads,state']

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def run(command, **kwargs):
    return subprocess.run(command, cwd=ROOT, text=True, check=True, **kwargs)

def identity(binary):
    OUT.mkdir(parents=True, exist_ok=True)
    name = Path(binary).name
    data = dict(binary=str(binary), sha256=sha(binary), head=run(['git','rev-parse','HEAD'],capture_output=True).stdout.strip(),
                diff=run(['git','diff'],capture_output=True).stdout,
                files={str(p.relative_to(ROOT)):sha(p) for base in ('src','tests') for p in sorted((ROOT/base).rglob('*.rs'))},
                adapter_sha256=sha(__file__), cargo_sha256=sha(ROOT/'Cargo.toml'), lock_sha256=sha(ROOT/'Cargo.lock'),
                rustc=run(['rustc','+1.96.0','-Vv'],capture_output=True).stdout,
                environment={k:os.environ.get(k) for k in ('CARGO_TARGET_DIR','CARGO_BUILD_JOBS','RUSTUP_AUTO_INSTALL','RUSTFLAGS')},
                build='cargo +1.96.0 test --offline --locked --release --features tool-json --test ct_schema --no-run --message-format=json')
    (OUT/f'{name}-identity.json').write_text(json.dumps(data,indent=2)+'\n')

def collect():
    from co_count_monitored import bad
    OUT.mkdir(parents=True, exist_ok=True)
    pre = TOP.copy(); pre[2] = '3'
    raw = run(pre, capture_output=True).stdout
    blocks = ['Processes:'+s for s in raw.split('Processes:')[1:]]
    owned = {os.getpid(), os.getppid()}
    assert len(blocks) == 3 and all(not bad(b,90,owned) for b in blocks[-2:]), 'busy preflight; no retry'
    monitor = subprocess.Popen(TOP, stdout=subprocess.PIPE, text=True)
    state: dict[str, Any] = dict(text=[], samples=[], rejected=False, active=None)
    def watch():
        block = ''; streak = 0
        def assess(b):
            nonlocal streak
            busy = bad(b,85,owned)
            streak = streak+1 if busy else 0
            state['samples'].append(dict(time=time.time(), busy=busy, streak=streak))
            if streak >= 2:
                state['rejected'] = True
                p = state['active']
                if p and p.poll() is None: p.terminate()
        assert monitor.stdout is not None
        for line in monitor.stdout:
            state['text'].append(line)
            if line.startswith('Processes:') and block: assess(block); block = ''
            block += line
        if block: assess(block)
    thread = threading.Thread(target=watch,daemon=True); thread.start()
    rows = []
    try:
        for campaign in ('ab','aa'):
            for backend in ('tape','native'):
                for case in CASES:
                    for position, arm in enumerate('ABBA'):
                        assert not state['rejected'], 'CPU guard rejected; no retry'
                        binary = SCRATCH / ('ct-a' if campaign == 'aa' or arm == 'A' else 'ct-b')
                        env = os.environ | dict(CT_CASE=case, CT_BACKEND=backend)
                        command = [str(binary),'request','--exact','--ignored','--nocapture']
                        p = subprocess.Popen(command,cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
                        owned.add(p.pid)
                        state['active'] = p
                        stdout,_ = p.communicate(); state['active'] = None
                        assert p.returncode == 0, stdout
                        samples = [json.loads(s[3:]) for s in stdout.splitlines() if s.startswith('CT ')]
                        assert len(samples) == 48
                        rows.append(dict(campaign=campaign,backend=backend,case=case,arm=arm,position=position,
                                         binary_sha256=sha(binary),pid=p.pid,command=command,stdout=stdout,samples=samples))
    finally:
        monitor.terminate(); monitor.wait(); thread.join(5)
        with gzip.open(OUT/'raw.json.gz','wt') as f: json.dump(rows,f)
        with gzip.open(OUT/'cpu.json.gz','wt') as f: json.dump(dict(pre=raw,text=''.join(state['text']),samples=state['samples'],rejected=state['rejected']),f)
    assert not state['rejected'] and len(rows) == 192
    reduce()

def reduce():
    with gzip.open(OUT/'raw.json.gz','rt') as f: rows=json.load(f)
    summary=[]
    for backend in ('tape','native'):
        for case in CASES:
            cell: dict[str, Any] = dict(backend=backend,case=case)
            for campaign in ('ab','aa'):
                groups={arm:[statistics.median(s['ns'] for s in r['samples']) for r in rows if r['campaign']==campaign and r['backend']==backend and r['case']==case and r['arm']==arm] for arm in 'AB'}
                assert all(len(v)==2 for v in groups.values())
                cell[campaign]=dict(a_ns=statistics.median(groups['A']), b_ns=statistics.median(groups['B']),
                                    ratio=statistics.median(groups['B'])/statistics.median(groups['A']),process_medians=groups)
            summary.append(cell)
    (OUT/'summary.json').write_text(json.dumps(dict(processes=len(rows),requests=sum(len(r['samples']) for r in rows),cells=summary),indent=2)+'\n')
    for r in summary: print(r['backend'],r['case'],round(r['ab']['ratio'],3),round(r['aa']['ratio'],3))

if __name__ == '__main__':
    os.chdir(ROOT)
    os.environ.update(CARGO_TARGET_DIR=str(ROOT/'target'),CARGO_BUILD_JOBS='1',RUSTUP_AUTO_INSTALL='0')
    assert os.path.ismount('/Volumes/M.2') and (ROOT/'target').is_symlink()
    with (SCRATCH/'minipy-perf-build.lock').open('a') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX)
        if sys.argv[1]=='command': run(sys.argv[2:])
        elif sys.argv[1]=='identity': identity(sys.argv[2])
        elif sys.argv[1]=='collect': collect()
        elif sys.argv[1]=='reduce': reduce()
        else: raise ValueError(sys.argv[1])
