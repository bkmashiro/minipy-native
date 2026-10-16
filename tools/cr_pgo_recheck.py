#!/usr/bin/env python3
"""One fixed CN holdout ABBA+A/A recheck, with coarse macOS CPU guard."""
import gzip
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import threading
import time
from typing import Any

from perf_workbench import ROOT, SCRATCH, identity, locked, sha

OUT = ROOT / 'evidence/cr-pgo'
WORK = SCRATCH / 'cn-pgo'
TOP = ['top', '-l', '0', '-s', '1', '-n', '12', '-o', 'cpu',
       '-stats', 'pid,command,cpu,time,threads']
HOLDOUT = ['branch', 'scalar_inputs', 'madd_loop', 'escaped_join', 'detach_wait']


def assess(block, idle_limit, owned):
    idle = re.search(r'([\d.]+)% idle', block)
    competitors = [dict(pid=int(pid), command=cmd, core_cpu_percent=float(cpu))
                   for pid, cmd, cpu in re.findall(
                       r'^\s*(\d+)\s+(\S+)\s+([\d.]+)\s', block, re.M)
                   if int(pid) not in owned and cmd not in ('baseline', 'candidate', 'top')]
    return dict(idle_percent=float(idle[1]) if idle else None,
                competitors=competitors,
                busy=not idle or float(idle[1]) < idle_limit or
                any(p['core_cpu_percent'] >= 50 for p in competitors))


def main():
    assert not OUT.exists(), 'one attempt only; retain failed attempts'
    OUT.mkdir(parents=True)
    raw: dict[str, Any] = dict(start_epoch=time.time(), policy=dict(pre_idle_min=90, during_idle_min=85,
               unrelated_core_cpu_max_exclusive=50, interval_seconds=1,
               stop='one busy valid sample; terminate only own benchmark process group'),
               records=[], monitor_samples=[])
    try:
        assert os.path.ismount('/Volumes/M.2')
        assert (ROOT / 'target').resolve() == Path('/Volumes/M.2/caches/minipy-native-target')
        profile = json.loads((ROOT / 'evidence/cn-pgo/profile.json').read_text())
        raw['profile_sha256'] = sha(WORK / 'merged.profdata')
        assert raw['profile_sha256'] == profile['profile_sha256']
        raw['arms'] = {n: identity(WORK / n) for n in ('baseline', 'candidate')}
        for name, meta in raw['arms'].items():
            expected = json.loads((ROOT / f'evidence/cn-pgo/{name}-identity.json').read_text())
            assert {k: v for k, v in meta.items() if k != 'path'} == expected
            assert all(sha(ROOT / p) == h for p, h in meta['sources'].items())
        raw['driver_sha256'] = sha(ROOT / 'tools/perf_workbench.py')
        raw['collector_sha256'] = sha(__file__)
        owned = {os.getpid(), os.getppid()}
        # Preflight inside shared lock, release before original driver acquires it.
        with locked():
            cmd = TOP.copy()
            cmd[2] = '3'
            pre = subprocess.run(cmd, capture_output=True, text=True, check=True)
            blocks = ['Processes:' + b for b in pre.stdout.split('Processes:')[1:]]
            assert len(blocks) == 3
            raw['preflight'] = dict(command=cmd, stdout=pre.stdout, epoch=time.time(),
                                    samples=[assess(b, 90, owned) for b in blocks[1:]])
            assert not any(s['busy'] for s in raw['preflight']['samples']), 'busy preflight'
        monitor = subprocess.Popen(TOP, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                   text=True, bufsize=1)
        owned.add(monitor.pid)
        rejected = threading.Event()
        lines = []

        def watch():
            assert monitor.stdout is not None
            block = ''
            began = None
            index = 0
            for line in monitor.stdout:
                lines.append(line)
                if line.startswith('Processes:'):
                    if block:
                        # Only completed blocks; first top sample is boot-average.
                        sample = dict(index=index, begin_epoch=began, end_epoch=time.time(),
                                      valid_delta=index > 0, **assess(block, 85, owned))
                        raw['monitor_samples'].append(sample)
                        if sample['valid_delta'] and sample['busy']:
                            rejected.set()
                        index += 1
                    block = ''
                    began = time.time()
                block += line
            # EOF block may be incomplete: preserve text, never assess it.

        thread = threading.Thread(target=watch, daemon=True)
        thread.start()
        try:
            # Establish actual interval deltas, not first-sample boot averages.
            time.sleep(2.2)
            for name, extra in [('ab', ['--candidate', str(WORK / 'candidate')]), ('aa', ['--aa'])]:
                assert not rejected.is_set(), 'busy before campaign'
                cmd = [sys.executable, 'tools/perf_workbench.py', 'run', '--baseline',
                       str(WORK / 'baseline'), '--cases', *HOLDOUT, '--rounds', '1',
                       '--reps', '12', '--output', str(OUT / f'{name}.jsonl.gz'), *extra]
                start = time.time()
                child = subprocess.Popen(cmd, cwd=ROOT, stdout=subprocess.PIPE,
                                         stderr=subprocess.STDOUT, text=True, start_new_session=True)
                # communicate(timeout) drains output and avoids a full pipe deadlock.
                while True:
                    try:
                        stdout, _ = child.communicate(timeout=0.1)
                        break
                    except subprocess.TimeoutExpired:
                        if rejected.is_set():
                            os.killpg(child.pid, signal.SIGTERM)
                            stdout, _ = child.communicate(timeout=10)
                            break
                raw['records'].append(dict(name=name, command=cmd, start_epoch=start,
                    end_epoch=time.time(), exit_code=child.returncode, stdout=stdout))
                print(name, 'exit', child.returncode, flush=True)
                assert child.returncode == 0 and not rejected.is_set(), 'busy/failed campaign'
            # Complete monitor intervals covering the end of the final campaign.
            time.sleep(2.2)
        finally:
            monitor.terminate()
            monitor.wait(timeout=10)
            thread.join(timeout=5)
            raw['monitor_command'] = TOP
            raw['monitor_text'] = ''.join(lines)
            raw['rejected'] = rejected.is_set()
            raw['monitor_thread_stopped'] = not thread.is_alive()
        assert not raw['rejected'] and raw['monitor_thread_stopped']
        for record in raw['records']:
            record['overlapping_valid_samples'] = [s['index'] for s in raw['monitor_samples']
                if s['valid_delta'] and s['begin_epoch'] <= record['end_epoch']
                and s['end_epoch'] >= record['start_epoch']]
            assert record['overlapping_valid_samples'], 'no during interval; inconclusive, do not repeat'
        for name, meta in raw['arms'].items():
            assert sha(WORK / name) == meta['binary_sha256']
        raw['complete'] = True
    finally:
        raw['end_epoch'] = time.time()
        with gzip.open(OUT / 'collection.json.gz', 'xt') as f:
            json.dump(raw, f, indent=2)


if __name__ == '__main__':
    main()