#!/usr/bin/env python3
"""One replacement CO pair, same binary/sample plan, with live CPU guard.
Unmonitored originals remain intact and are never pooled with this collection.
"""
import gzip
import json
import os
import re
import subprocess
import threading
import time
from typing import Any
from co_count_collect import OUT
from perf_workbench import ROOT, SCRATCH, sha

TOP = ['top', '-l', '0', '-s', '1', '-n', '8', '-o', 'cpu', '-stats', 'pid,command,cpu,time,threads,state']
CASES = ['count_price_small', 'count_price_large', 'count_quantity_small', 'count_quantity_large', 'count_none', 'count_all', 'scalar', 'quote', 'ready']


def bad(block, minimum_idle, owned):
    idle = re.findall(r'([\d.]+)% idle', block)
    if not idle:
        return True
    busy = []
    for pid, command, cpu in re.findall(r'^\s*(\d+)\s+(\S+)\s+([\d.]+)\s', block, re.M):
        if int(pid) not in owned and command not in ('co-a', 'co-b', 'top') and float(cpu) >= 50:
            busy.append((int(pid), command, float(cpu)))
    return float(idle[-1]) < minimum_idle or bool(busy)


def main():
    path = OUT / 'cpu.json.gz'
    assert not path.exists()
    for name in ('monitored-ab', 'monitored-aa'):
        assert not (OUT / f'{name}.jsonl.gz').exists()
    pre_command = TOP.copy()
    pre_command[2] = '3'
    pre = subprocess.run(pre_command, capture_output=True, text=True, check=True)
    blocks = ['Processes:' + b for b in pre.stdout.split('Processes:')[1:]]
    assert len(blocks) == 3
    owned = {os.getpid(), os.getppid()}
    ready = all(not bad(b, 90, owned) for b in blocks[-2:])
    raw: dict[str, Any] = dict(pre_command=pre_command, pre=pre.stdout, pre_exit=pre.returncode, ready=ready,
               policy='last two preflight samples >=90% global idle and no unrelated process >=50% core CPU; during timing stop own campaign on two consecutive samples below 85% idle or unrelated >=50%; no kill of unrelated processes',
               binaries={arm: sha(SCRATCH / f'co-{arm.lower()}') for arm in 'AB'}, records=[])
    if not ready:
        with gzip.open(path, 'xt') as f:
            json.dump(raw, f, indent=2)
        raise RuntimeError('preflight busy: stop collection, do not retry')
    monitor = subprocess.Popen(TOP, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, bufsize=1)
    owned.add(monitor.pid)
    state: dict[str, Any] = dict(lines=[], samples=[], busy_streak=0, rejected=False, active=None)
    def watch():
        def assess(block):
            if block:
                busy = bad(block, 85, owned)
                state['busy_streak'] = state['busy_streak'] + 1 if busy else 0
                state['samples'].append(dict(time=time.time(), during_process=state['active'].pid if state['active'] else None, busy=busy, streak=state['busy_streak']))
                if state['busy_streak'] >= 2:
                    state['rejected'] = True
                    if state['active'] and state['active'].poll() is None:
                        state['active'].terminate()
        block = ''
        assert monitor.stdout is not None
        for line in monitor.stdout:
            state['lines'].append(line)
            if line.startswith('Processes:') and block:
                assess(block)
                block = ''
            block += line
        assess(block)
    thread = threading.Thread(target=watch, daemon=True)
    thread.start()
    try:
        base = ['python3', 'tools/perf_workbench.py', 'run', '--baseline', str(SCRATCH / 'co-a'), '--rounds', '1', '--reps', '12', '--cases', *CASES]
        for name, args in [('monitored-ab', ['--candidate', str(SCRATCH / 'co-b')]), ('monitored-aa', ['--aa'])]:
            if state['rejected']:
                break
            command = base + args + ['--output', str(OUT / f'{name}.jsonl.gz')]
            r = subprocess.Popen(command, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
            state['active'] = r
            stdout, _ = r.communicate()
            raw['records'].append(dict(command=command, exit_code=r.returncode, stdout=stdout))
            state['active'] = None
            print(name, 'exit', r.returncode, flush=True)
            if r.returncode:
                break
    finally:
        monitor.terminate()
        monitor.wait()
        thread.join(timeout=5)
        raw.update(monitor_command=TOP, monitor_text=''.join(state['lines']), samples=state['samples'], rejected=state['rejected'])
        with gzip.open(path, 'xt') as f:
            json.dump(raw, f, indent=2)
    assert not state['rejected'] and len(raw['records']) == 2 and all(r['exit_code'] == 0 for r in raw['records'])
    assert len(state['samples']) >= 2
    print('CPU samples', len(state['samples']), 'busy', sum(s['busy'] for s in state['samples']))

if __name__ == '__main__':
    main()
