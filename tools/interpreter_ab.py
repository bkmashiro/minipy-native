"""G small serial three-engine Linux campaign. No builds or VM configuration.
Usage: python3 tools/interpreter_ab.py HOST KEY KNOWN_HOSTS
Requires target/g-{speed,memory}-linux and target/g-test-build.jsonl.
"""
import gzip
import hashlib
import json
from pathlib import Path
import shlex
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]

def run(args):
    return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT)

def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()

def main():
    host, key, known = sys.argv[1:]
    opts = ['-i', key, '-o', f'UserKnownHostsFile={known}', '-o', 'StrictHostKeyChecking=yes', '-o', 'BatchMode=yes']
    ssh = ['ssh', *opts, host]
    scp = ['scp', *opts]
    remote = '/root/minipy-native-g-interpreter'
    out = ROOT / 'bench/results/g-interpreter'
    out.mkdir(parents=True, exist_ok=True)
    assert not (out / 'linux.jsonl.gz').exists(), 'preserve raw'
    identity = {'baseline': run(['git', 'rev-parse', 'HEAD']).strip(), 'rust_core': run(['rustc', '--version']).strip(), 'rust_bench': run(['rustc', '+1.96.0', '--version']).strip(), 'zig': run(['zig', 'version']).strip(), 'target': 'aarch64-unknown-linux-musl', 'initial_vm': 'off; F released; started existing launcher with PTY', 'preflight': run(ssh + ['uname -a; uptime; free -m; df -h /; ps']), 'commands': [], 'binaries': {}, 'source_sha256': {}, 'summary_window': 'sample >= 101; 201 samples per process; last 100 chosen before execution; all raw retained', 'boundary': 'resident single-thread runtime, no per-request process; cold parse+prepare+producer/import+execution+owned-code reclamation; no source cache; warm explicit same-code reuse, Monty clone included; stdout consumed/oracle and JSON outside timing; input scalar schema/tools fixed'}
    files = ['Cargo.toml', 'Cargo.lock', 'bench/Cargo.toml', 'bench/Cargo.lock', 'src/interpreter.rs', 'src/runtime.rs', 'src/codegen.rs', 'src/frontend.rs', 'src/ir.rs', 'src/buffer.rs', 'src/provider.rs', 'tests/interpreter.rs', 'bench/src/bin/interpreter_ab.rs', 'bench/src/v2_common.rs', 'tools/interpreter_ab.py']
    files += [str(p.relative_to(ROOT)) for p in sorted((ROOT / 'fixtures').glob('*.py'))]
    identity['source_sha256'] = {p: sha(ROOT / p) for p in files}
    run(ssh + [f'mkdir -p {remote}'])
    for mode in ['speed', 'memory']:
        p = ROOT / f'target/g-{mode}-linux'
        run(scp + [str(p), f'{host}:{remote}/{mode}'])
        assert run(ssh + [f'sha256sum {remote}/{mode}']).split()[0] == sha(p)
        identity['binaries'][mode] = {'sha256': sha(p), 'bytes': p.stat().st_size}
    build = [json.loads(l) for l in (ROOT / 'target/g-test-build.jsonl').read_text().splitlines()]
    artifacts = [Path(r['executable']) for r in build if r.get('reason') == 'compiler-artifact' and r.get('executable') and r.get('profile', {}).get('test') and r['target']['kind'] != ['bin']]
    assert len(artifacts) == 5, artifacts
    log = ''
    for p in artifacts:
        run(scp + [str(p), f'{host}:{remote}/{p.name}'])
        cmd = f'{remote}/{p.name} --test-threads=1'
        identity['commands'].append(cmd)
        log += cmd + '\n' + run(ssh + [cmd])
    (out / 'linux-tests.txt').write_text(log)
    count = 0
    shapes = [('scalar', 0), ('weighted', 100), ('weighted', 10000), ('stateful', 100), ('stateful', 10000)]
    with gzip.open(out / 'linux.jsonl.gz', 'wt') as f:
        for round_id in range(3):
            engines = ['native', 'interpreter', 'monty']
            engines = engines[round_id:] + engines[:round_id]
            for case, n in (shapes if round_id % 2 == 0 else list(reversed(shapes))):
                for mode in ['cold', 'warm']:
                    for engine in engines:
                        cmd = shlex.join([f'{remote}/speed', engine, case, mode, str(n), '201'])
                        identity['commands'].append(cmd)
                        rows = [json.loads(l) for l in run(ssh + [cmd]).splitlines()]
                        assert len(rows) == 201 and all(r['completed'] for r in rows)
                        for r in rows:
                            r['round'] = round_id
                            f.write(json.dumps(r, separators=(',', ':')) + '\n')
                            count += 1
                        f.flush()
            print('speed round', round_id, 'completed', flush=True)
    assert count == 18090
    with (out / 'memory.jsonl').open('w') as f:
        for n, tasks in [(100, 32), (10000, 8)]:
            for engine in ['native', 'interpreter', 'monty']:
                cmd = shlex.join([f'{remote}/memory', engine, 'weighted', 'hold', str(n), str(tasks)])
                identity['commands'].append(cmd)
                rows = [json.loads(l) for l in run(ssh + [cmd]).splitlines()]
                assert [r['phase'] for r in rows] == ['startup', 'load_wait', 'record_wait', 'reclaimed']
                for r in rows:
                    f.write(json.dumps(r, separators=(',', ':')) + '\n')
                print('memory', n, engine, 'completed', flush=True)
    identity['raw_rows'] = count
    identity['postflight'] = run(ssh + ['uptime; free -m; df -h /; ps'])
    (out / 'identity.json').write_text(json.dumps(identity, indent=2) + '\n')

if __name__ == '__main__':
    main()
