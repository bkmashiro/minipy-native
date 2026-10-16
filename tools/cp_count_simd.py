#!/usr/bin/env python3
"""One bounded CO-scalar vs LLVM-vectorized count experiment; no tuning loop."""
import argparse
import gzip
import hashlib
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

from co_count_collect import OBJDUMP
from co_count_monitored import CASES, TOP
from ce_overflow_reduce import apply
from perf_workbench import ROOT, SCRATCH, identity, locked, reduce, sha

OUT = ROOT / 'evidence/cp-count'
BASE = '71582f7bdb9408d62a1838f2face9e0b24f21492'
ARMS = {'A': SCRATCH / 'co-b', 'B': SCRATCH / 'cp-simd'}
ENV = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / 'target'), CARGO_BUILD_JOBS='1', RUSTUP_AUTO_INSTALL='0')


def save(name, value):
    OUT.mkdir(parents=True, exist_ok=True)
    with gzip.open(OUT / name, 'xt') as f:
        json.dump(value, f, indent=2)


def assess(block, idle_limit):
    idle = re.search(r'([\d.]+)% idle', block)
    competitors = [dict(pid=int(pid), command=cmd, cpu=float(cpu))
                   for pid, cmd, cpu in re.findall(r'^\s*(\d+)\s+(\S+)\s+([\d.]+)\s', block, re.M)
                   if int(pid) not in (os.getpid(), os.getppid()) and cmd not in ('co-b', 'cp-simd', 'top')]
    return dict(idle=float(idle[1]) if idle else None, competitors=competitors,
                busy=not idle or float(idle[1]) < idle_limit or any(p['cpu'] >= 50 for p in competitors))


def collect():
    assert not (OUT / 'collection.json.gz').exists(), 'one attempt only'
    assert not any((OUT / f'{n}.jsonl.gz').exists() for n in ('ab', 'aa'))
    assert not any(ENV.get(k) for k in ('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS'))
    raw: dict[str, Any] = dict(start=time.time(), arms={k: identity(v) for k, v in ARMS.items()},
               collector_sha256=sha(__file__), driver_sha256=sha(ROOT / 'tools/perf_workbench.py'),
               policy='pre: last two valid idle>=90%, during: every valid idle>=85%; unrelated process <50% core; stop own process group on first busy delta; one attempt',
               records=[], samples=[])
    try:
        for p, h in raw['arms']['A']['sources'].items():
            blob = subprocess.check_output(['git', 'show', f'{BASE}:{p}'], cwd=ROOT)
            assert hashlib.sha256(blob).hexdigest() == h, p
        assert all(sha(ROOT / p) == h for p, h in raw['arms']['B']['sources'].items())
        assert {p for p in raw['arms']['A']['sources'] if raw['arms']['A']['sources'][p] != raw['arms']['B']['sources'][p]} == {'src/interpreter/tape.rs'}
        assert os.path.ismount('/Volumes/M.2') and (ROOT / 'target').resolve() == Path('/Volumes/M.2/caches/minipy-native-target')
        with locked():
            cmd = TOP.copy()
            cmd[2] = '3'
            pre = subprocess.run(cmd, capture_output=True, text=True, check=True)
            blocks = ['Processes:' + b for b in pre.stdout.split('Processes:')[1:]]
            assert len(blocks) == 3
            raw['pre'] = dict(command=cmd, text=pre.stdout, samples=[assess(b, 90) for b in blocks[1:]])
            assert not any(s['busy'] for s in raw['pre']['samples']), 'busy preflight; stop, no repeat'
        monitor = subprocess.Popen(TOP, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, bufsize=1)
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
                        sample = dict(index=index, begin=began, end=time.time(), valid=index > 0, **assess(block, 85))
                        raw['samples'].append(sample)
                        if sample['valid'] and sample['busy']:
                            rejected.set()
                        index += 1
                    block = ''
                    began = time.time()
                block += line
            # Preserve EOF text, but never promote a possibly partial block.

        thread = threading.Thread(target=watch, daemon=True)
        thread.start()
        try:
            time.sleep(2.2)  # Establish actual interval samples, not boot-average first top output.
            for name, extra in [('ab', ['--candidate', str(ARMS['B'])]), ('aa', ['--aa'])]:
                assert not rejected.is_set(), 'busy before campaign'
                cmd = [sys.executable, 'tools/perf_workbench.py', 'run', '--baseline', str(ARMS['A']),
                       '--cases', *CASES, '--rounds', '1', '--reps', '12', '--output', str(OUT / f'{name}.jsonl.gz'), *extra]
                start = time.time()
                child = subprocess.Popen(cmd, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, start_new_session=True)
                while True:
                    try:
                        stdout, _ = child.communicate(timeout=0.1)
                        break
                    except subprocess.TimeoutExpired:
                        if rejected.is_set():
                            os.killpg(child.pid, signal.SIGTERM)
                            stdout, _ = child.communicate(timeout=10)
                            break
                raw['records'].append(dict(name=name, command=cmd, start=start, end=time.time(), exit_code=child.returncode, stdout=stdout))
                print(name, 'exit', child.returncode, flush=True)
                assert child.returncode == 0 and not rejected.is_set(), 'busy/failed campaign; stop'
            time.sleep(2.2)  # Close monitor intervals covering the final requests.
        finally:
            monitor.terminate()
            monitor.wait(timeout=10)
            thread.join(timeout=5)
            raw.update(monitor_command=TOP, monitor_text=''.join(lines), rejected=rejected.is_set(), thread_stopped=not thread.is_alive())
        assert not raw['rejected'] and raw['thread_stopped']
        assert any(s['valid'] for s in raw['samples'])
        for arm, path in ARMS.items():
            assert sha(path) == raw['arms'][arm]['binary_sha256']
        raw['complete'] = True
    finally:
        raw['end'] = time.time()
        save('collection.json.gz', raw)


def verify():
    assert not (OUT / 'verification.json.gz').exists()
    cargo = ['cargo', '+1.96.0', 'test', '--offline', '--locked', '--release']
    clippy = ['cargo', '+stable', 'clippy', '--offline', '--locked', '--release', '--all-targets']
    commands = [cargo, cargo + ['--features', 'tool-json'], cargo + ['--features', 'tool-json,debug-runtime'],
                clippy + ['--', '-D', 'warnings'], clippy + ['--features', 'tool-json', '--', '-D', 'warnings'],
                clippy + ['--features', 'tool-json,debug-runtime', '--', '-D', 'warnings'],
                ['cargo', '+stable', 'fmt', '--all', '--', '--check'],
                [sys.executable, '-m', 'unittest', 'discover', '-s', 'tools', '-p', 'test_perf_workbench.py', '-v'],
                ['git', 'diff', '--check']]
    records = []
    with locked():
        for cmd in commands:
            r = subprocess.run(cmd, cwd=ROOT, env=ENV, text=True, capture_output=True)
            records.append(dict(command=cmd, exit_code=r.returncode, stdout=r.stdout, stderr=r.stderr))
            print('exit', r.returncode, cmd, flush=True)
            if r.returncode:
                print(r.stdout, r.stderr)
                break
    save('verification.json.gz', dict(tape_sha256=sha(ROOT / 'src/interpreter/tape.rs'), records=records))
    assert len(records) == len(commands) and all(r['exit_code'] == 0 for r in records)


def machine():
    assert not (OUT / 'machine.json.gz').exists()
    result = {}
    with locked():
        for arm, binary in ARMS.items():
            symbols = subprocess.check_output([OBJDUMP, '--syms', str(binary)], text=True)
            selected = sorted({s.split()[-1] for s in symbols.splitlines() if ' F ' in s and 'interpreter4tape' in s})
            rows = []
            for symbol in selected:
                cmd = [OBJDUMP, '--disassemble-symbols=' + symbol, str(binary)]
                assembly = subprocess.check_output(cmd, text=True)
                instructions = re.findall(r'^\s*[0-9a-f]+:\s+(.*)$', assembly, re.M)
                rows.append(dict(symbol=symbol, command=cmd, instructions=len(instructions), bytes=4 * len(instructions), assembly=assembly))
            result[arm] = dict(binary_sha256=sha(binary), binary_bytes=binary.stat().st_size, rows=rows)
    save('machine.json.gz', result)


def replay():
    with gzip.open(OUT / 'collection.json.gz', 'rt') as f:
        collection = json.load(f)
    with gzip.open(OUT / 'collector.py.gz', 'rb') as f:
        assert hashlib.sha256(f.read()).hexdigest() == collection['collector_sha256']
    assert collection['complete'] and not collection['rejected']
    assert len(collection['records']) == 2 and all(r['exit_code'] == 0 for r in collection['records'])
    assert all(not s['busy'] for s in collection['pre']['samples'])
    assert all(not s['busy'] for s in collection['samples'] if s['valid'])
    pre = ['Processes:' + b for b in collection['pre']['text'].split('Processes:')[1:]]
    assert len(pre) == 3 and all(not assess(b, 90)['busy'] for b in pre[1:])
    blocks = ['Processes:' + b for b in collection['monitor_text'].split('Processes:')[1:]]
    for s in collection['samples']:
        assert s['idle'] == assess(blocks[s['index']], 85)['idle']
        if s['valid']:
            assert not assess(blocks[s['index']], 85)['busy']
    patch = (OUT / 'candidate.patch').read_text()
    assert patch.count('diff --git ') == 1 and patch.startswith('diff --git a/src/interpreter/tape.rs b/src/interpreter/tape.rs\n')
    for p, h in collection['arms']['A']['sources'].items():
        source = subprocess.check_output(['git', 'show', f'{BASE}:{p}'], cwd=ROOT)
        assert hashlib.sha256(source).hexdigest() == h
        candidate = apply(source.decode(), patch).encode() if p == 'src/interpreter/tape.rs' else source
        assert hashlib.sha256(candidate).hexdigest() == collection['arms']['B']['sources'][p]
        # Final production state is the original reviewed scalar, not dead SIMD.
        assert sha(ROOT / p) == h
    # top delta describes the preceding interval. Receipt timestamps are only
    # approximate, not a per-request scheduling/exclusivity proof.
    samples = collection['samples']
    coverage = {r['name']: [s['index'] for i, s in enumerate(samples) if s['valid'] and i > 0
                           and samples[i - 1]['begin'] <= r['end'] and s['begin'] >= r['start']]
                for r in collection['records']}
    assert len(coverage) == 2 and all(coverage.values())
    summary = dict(base=BASE, coverage=coverage, timing={})
    for name in ('ab', 'aa'):
        s = reduce(OUT / f'{name}.jsonl.gz')
        assert s['complete'] and s['measured_samples'] == 864 and s['observed_processes'] == 72
        with gzip.open(OUT / f'{name}.jsonl.gz', 'rt') as f:
            manifest = json.loads(next(f))
        assert manifest['driver_sha256'] == sha(ROOT / 'tools/perf_workbench.py')
        assert manifest['rounds'] == 1 and manifest['reps'] == 12 and not manifest['diagnostic']
        assert manifest['aa'] == (name == 'aa') and manifest['order'] == 'ABBA'
        assert manifest['entries'] == {'A': 'registered', 'B': 'registered'}
        assert {tuple(c) for c in manifest['cells']} == {(b, c) for b in ('tape', 'native') for c in CASES}
        assert manifest['arms']['A'] == collection['arms']['A']
        assert manifest['arms']['B'] == collection['arms']['A' if name == 'aa' else 'B']
        summary['timing'][name] = s
    with gzip.open(OUT / 'machine.json.gz', 'rt') as f:
        asm = json.load(f)
    summary['machine'] = {}
    for arm in 'AB':
        assert asm[arm]['binary_sha256'] == collection['arms'][arm]['binary_sha256']
        for row in asm[arm]['rows']:
            assert row['instructions'] == len(re.findall(r'^\s*[0-9a-f]+:', row['assembly'], re.M))
            assert row['bytes'] == row['instructions'] * 4
        kernels = [r for r in asm[arm]['rows'] if ('scalar_count' if arm == 'A' else 'count_chunk') in r['symbol']]
        assert len(kernels) == 2
        assert all(('cmgt.2d' in k['assembly']) == (arm == 'B') for k in kernels)
        summary['machine'][arm] = dict(binary_bytes=asm[arm]['binary_bytes'],
            linked_tape_bytes=sum(r['bytes'] for r in asm[arm]['rows']),
            kernels=[dict(symbol=k['symbol'], instructions=k['instructions'], bytes=k['bytes']) for k in kernels])
    with gzip.open(OUT / 'verification.json.gz', 'rt') as f:
        verification = json.load(f)
    assert len(verification['records']) == 9 and all(r['exit_code'] == 0 for r in verification['records'])
    assert verification['tape_sha256'] == collection['arms']['B']['sources']['src/interpreter/tape.rs']
    text = json.dumps(summary, indent=2) + '\n'
    path = OUT / 'summary.json'
    if path.exists():
        assert path.read_text() == text, 'replay changed summary'
    else:
        path.write_text(text)
    print(text)


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('action', choices=['verify', 'machine', 'collect', 'replay'])
    globals()[p.parse_args().action]()
