"""L frozen campaign. Usage: final_campaign.py HOST KEY KNOWN SPEED MEMORY.
Plan registered in bench/results/l-final/plan.json; refuses raw replacement.
"""
import gzip
import hashlib
import json
from pathlib import Path
import shlex
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'bench/results/l-final'
PLAN = json.loads((OUT / 'plan.json').read_text())


def run(argv):
    return subprocess.check_output(argv, text=True, stderr=subprocess.STDOUT, timeout=240)


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    host, key, known, speed, memory = sys.argv[1:]
    opts = ['-i', key, '-o', f'UserKnownHostsFile={known}',
            '-o', 'StrictHostKeyChecking=yes', '-o', 'BatchMode=yes']
    ssh = ['ssh', *opts, host]
    remote = '/root/minipy-native-l'
    raw = OUT / 'speed.jsonl.gz'
    assert not raw.exists(), 'preserve raw; use explicit new campaign for a rerun'
    identity = {'runtime_commit': run(['git', 'rev-parse', 'HEAD']).strip(),
                'harness_patch': run(['git', 'diff', '--', 'bench/src/bin/interpreter_ab.rs']),
                'rust': run(['rustc', '+1.96.0', '--version']).strip(),
                'zig': run(['zig', 'version']).strip(), 'commands': [], 'binaries': {},
                'preflight': run(ssh + ['uname -a; uptime; free -m; df -h /; ps']),
                'source_sha256': {str(p.relative_to(ROOT)): sha(p)
                                  for pattern in ['src/*.rs', 'fixtures/*.py', 'bench/src/*.rs',
                                                  'bench/src/bin/*.rs', 'Cargo*', 'bench/Cargo*',
                                                  'tools/final_*.py', 'bench/results/l-final/plan.json']
                                  for p in ROOT.glob(pattern) if p.is_file()},
                'scheduled_commands': 414, 'completed_commands': 0, 'failed_commands': []}
    assert identity['runtime_commit'] == PLAN['runtime_commit']
    for name, p in [('speed', speed), ('memory', memory)]:
        run(['scp', *opts, p, f'{host}:{remote}/{name}'])
        assert run(ssh + [f'sha256sum {remote}/{name}']).split()[0] == sha(p)
        identity['binaries'][name] = {'sha256': sha(p), 'bytes': Path(p).stat().st_size}
    # Small real smoke exercises every backend, semaphore and shared-owner gate.
    with (OUT / 'smoke.jsonl').open('w') as f:
        for engine in PLAN['primary_engines']:
            for binary, case, mode, n, reps, env in [
                    ('speed', 'weighted', 'cold', 100, 2, ''),
                    ('speed', 'scan', 'mixed-cold', 0, 1, 'MINIPY_INFLIGHT=8 MINIPY_TOOL_DELAY_US=1000'),
                    ('memory', 'weighted', 'hold', 100, 1, 'MINIPY_HOLD_CODE=shared')]:
                cmd = env + ' ' + shlex.join([f'{remote}/{binary}', engine, case, mode, str(n), str(reps)])
                f.write(run(ssh + [cmd]))
    count = mem_count = 0
    try:
        with gzip.open(raw, 'wt') as speed_file, (OUT / 'memory.jsonl').open('w') as memory_file:
            def collect(binary, engine, case, mode, n, reps, round_id, delay=0, inflight=40, code='unique'):
                nonlocal count, mem_count
                cmd = f'MINIPY_CHUNK=256 MINIPY_TOOL_DELAY_US={delay} MINIPY_INFLIGHT={inflight} MINIPY_HOLD_CODE={code} ' + shlex.join([f'{remote}/{binary}', engine, case, mode, str(n), str(reps)])
                identity['commands'].append(cmd)
                try:
                    rows = [json.loads(l) for l in run(ssh + [cmd]).splitlines()]
                    expected = 4 if binary == 'memory' else reps * (40 if mode.startswith('mixed-') else 1)
                    assert len(rows) == expected
                    if binary == 'memory':
                        assert [r['phase'] for r in rows] == PLAN['memory']['phases']
                    else:
                        assert all(r['completed'] for r in rows)
                        assert sorted({r['sample'] for r in rows}) == list(range(reps))
                    for r in rows:
                        r.update(round=round_id, chunk=256)
                        (memory_file if binary == 'memory' else speed_file).write(json.dumps(r, separators=(',', ':')) + '\n')
                    speed_file.flush(); memory_file.flush()
                    if binary == 'memory': mem_count += len(rows)
                    else: count += len(rows)
                    identity['completed_commands'] += 1
                except Exception as exc:
                    identity['failed_commands'].append({'command': cmd, 'error': str(exc)})
                    raise
            for ri in range(PLAN['rounds']):
                engines = PLAN['primary_engines']
                order = engines[ri:] + engines[:ri]
                if ri % 2: order.reverse()
                for case, n in PLAN['shapes']:
                    for mode in PLAN['shape_modes']:
                        for engine in order: collect('speed', engine, case, mode, n, PLAN['shape_samples'], ri)
                for mix in ['light', 'scan']:
                    for mode, delay in PLAN['mixed']['modes']:
                        for engine in order: collect('speed', engine, mix, mode, 0, PLAN['mixed']['samples'], ri, delay)
                for inflight in [1, 8, 32]:
                    for engine in order: collect('speed', engine, 'scan', 'mixed-cold', 0, PLAN['mixed']['samples'], ri, 1000, inflight)
                print('speed round', ri, 'complete', count, flush=True)
            for ri in range(3):
                engines = PLAN['memory']['engines']
                order = engines[ri:] + engines[:ri]
                if ri % 2: order.reverse()
                for code in PLAN['memory']['code']:
                    for tasks in PLAN['memory']['tasks']:
                        for engine in order: collect('memory', engine, 'weighted', 'hold', 10000, tasks, ri, code=code)
                print('memory round', ri, 'complete', mem_count, flush=True)
        assert count == PLAN['speed_expected_rows']
        assert mem_count == PLAN['memory_expected_rows']
        # 3*(72 shape +24 mixed +12 capacity +30 memory) = 414 commands.
        identity['scheduled_commands'] = 414
        assert identity['completed_commands'] == 414
    finally:
        identity.update(speed_rows=count, memory_rows=mem_count,
                        postflight=run(ssh + ['uptime; free -m; df -h /; ps']))
        (OUT / 'identity.json').write_text(json.dumps(identity, indent=2) + '\n')


if __name__ == '__main__':
    main()
