"""H serial bounded campaign. Usage: python3 tools/average_ab.py HOST KEY KNOWN_HOSTS
Build target/h-{base,candidate}-{speed,memory}-linux first; never run concurrent VM work.
Two preregistered mixes: light [24,8,4,2,2], scan [8,16,8,4,4]; shapes
scalar/weighted100/stateful100/weighted10k/stateful100k. No automatic backend choice.
"""
import gzip
import hashlib
import json
from pathlib import Path
import shlex
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'bench/results/h-average'

def run(args):
    return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT)

def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()

def main():
    host, key, known = sys.argv[1:]
    opts = ['-i', key, '-o', f'UserKnownHostsFile={known}', '-o', 'StrictHostKeyChecking=yes', '-o', 'BatchMode=yes']
    ssh = ['ssh', *opts, host]
    scp = ['scp', *opts]
    remote = '/root/minipy-native-h-average'
    OUT.mkdir(parents=True, exist_ok=True)
    assert not (OUT / 'linux.jsonl.gz').exists(), 'preserve raw'
    identity = {
        'integrated_baseline': run(['git', 'rev-parse', 'HEAD']).strip(),
        'parents': run(['git', 'show', '-s', '--format=%P', 'HEAD']).strip(),
        'rust_core': run(['rustc', '--version']).strip(),
        'rust_bench': run(['rustc', '+1.96.0', '--version']).strip(),
        'zig': run(['zig', 'version']).strip(), 'target': 'aarch64-unknown-linux-musl',
        'vm_initial': 'off, started existing 2-vCPU/2048-MiB vfkit launcher',
        'preflight': run(ssh + ['uname -a; uptime; free -m; df -h /; ps']),
        'commands': [], 'binaries': {},
        'mixed_window': 'sample >= 3 out of 21 batches, three rounds; all raw retained',
        'shape_window': 'sample >= 21 out of 101 requests, three rounds',
        'mixes': {'light': [24, 8, 4, 2, 2], 'scan': [8, 16, 8, 4, 4]},
        'shapes': [['scalar', 0], ['weighted', 100], ['stateful', 100], ['weighted', 10000], ['stateful', 100000]],
        'arrival': '40 planned arrivals per 100ms batch, 500us apart, shape permutation (arrival*17)%40; overload if batch exceeds 100ms; no outstanding work crosses batches',
        'boundary': 'resident current-thread Tokio, source resident, no source cache; cold independent parse/prepare/execute/code drop; warm explicitly prepared five units outside timing; process CPU includes all 40 task futures, provider production/import/conversion/teardown and timer/queue overhead. Oracle precomputed outside; assertion inside; JSON outside. Speed has no allocation instrumentation. Provider delays 0/1000us; hold gates separate diagnostic binary.',
    }
    identity['source_sha256'] = {str(p.relative_to(ROOT)): sha(p) for p in sorted(ROOT.glob('src/*.rs'))}
    for s in ['Cargo.toml', 'Cargo.lock', 'bench/Cargo.toml', 'bench/Cargo.lock', 'bench/src/bin/interpreter_ab.rs', 'bench/src/v2_common.rs', 'tools/average_ab.py', 'tests/interpreter.rs']:
        identity['source_sha256'][s] = sha(ROOT / s)
    for p in ROOT.glob('fixtures/*.py'):
        identity['source_sha256'][str(p.relative_to(ROOT))] = sha(p)
    patch = ROOT / 'target/h-candidate.patch'
    (OUT / 'candidate.patch').write_bytes(patch.read_bytes())
    identity['candidate_patch_sha256'] = sha(patch)
    run(ssh + [f'mkdir -p {remote}'])
    for build in ['base', 'candidate']:
        for mode in ['speed', 'memory']:
            p = ROOT / f'target/h-{build}-{mode}-linux'
            name = f'{build}-{mode}'
            run(scp + [str(p), f'{host}:{remote}/{name}'])
            assert run(ssh + [f'sha256sum {remote}/{name}']).split()[0] == sha(p)
            identity['binaries'][name] = {'sha256': sha(p), 'bytes': p.stat().st_size}
    configs = [('base', 'native'), ('base', 'interpreter'), ('candidate', 'interpreter'), ('base', 'monty')]
    count = 0
    with gzip.open(OUT / 'linux.jsonl.gz', 'wt') as f:
        def collect(build, engine, case, mode, n, reps, round_id, delay=0):
            nonlocal count
            cmd = f'MINIPY_TOOL_DELAY_US={delay} ' + shlex.join([f'{remote}/{build}-speed', engine, case, mode, str(n), str(reps)])
            identity['commands'].append(cmd)
            rows = [json.loads(l) for l in run(ssh + [cmd]).splitlines()]
            assert len(rows) == reps * (40 if mode.startswith('mixed-') else 1)
            assert all(r['completed'] for r in rows)
            for r in rows:
                r.update(build=build, round=round_id)
                f.write(json.dumps(r, separators=(',', ':')) + '\n')
            count += len(rows)
            f.flush()
        for round_id in range(3):
            order = configs[round_id:] + configs[:round_id]
            if round_id == 1:
                order.reverse()
            for mix in ['light', 'scan']:
                for mode, delay in [('mixed-cold', 0), ('mixed-cold', 1000), ('mixed-warm', 0)]:
                    for build, engine in order:
                        collect(build, engine, mix, mode, 0, 21, round_id, delay)
            print('mixed round', round_id, 'complete', flush=True)
            for case, n in identity['shapes']:
                for mode in ['cold', 'warm']:
                    for build, engine in order:
                        collect(build, engine, case, mode, n, 101, round_id)
            print('shape round', round_id, 'complete', flush=True)
    assert count == 72600, count
    with (OUT / 'memory.jsonl').open('w') as f:
        for mix in ['light', 'scan']:
            for build, engine in configs + [('base', 'monty-clear')]:
                cmd = shlex.join([f'{remote}/{build}-memory', engine, mix, 'mixed-hold', '0', '1'])
                identity['commands'].append(cmd)
                rows = [json.loads(l) for l in run(ssh + [cmd]).splitlines()]
                assert [r['phase'] for r in rows] == ['startup', 'load_wait', 'record_wait', 'reclaimed']
                for r in rows:
                    r.update(build=build)
                    f.write(json.dumps(r, separators=(',', ':')) + '\n')
                print('memory', mix, build, engine, 'complete', flush=True)
    identity['raw_rows'] = count
    identity['postflight'] = run(ssh + ['uptime; free -m; df -h /; ps'])
    (OUT / 'identity.json').write_text(json.dumps(identity, indent=2) + '\n')

if __name__ == '__main__':
    main()
