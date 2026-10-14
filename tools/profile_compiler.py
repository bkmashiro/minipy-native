"""Bounded serial E compiler profile; no builds or host configuration changes.
python3 tools/profile_compiler.py HOST KEY KNOWN_HOSTS [HOST_KEY_ALIAS]
Requires target/e-profile-{plain,diagnostic}-linux from the documented builds.
"""
import gzip
import hashlib
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CASES = ['empty', 'scalar', 'scan', 'weighted', 'two_scan', 'legacy']


def run(args):
    return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    host, key, known, *alias = sys.argv[1:]
    opts = ['-i', key, '-o', f'UserKnownHostsFile={known}', '-o',
            'StrictHostKeyChecking=yes', '-o', 'BatchMode=yes']
    if alias:
        opts += ['-o', f'HostKeyAlias={alias[0]}']
    ssh = ['ssh', *opts, host]
    scp = ['scp', *opts]
    out = ROOT / 'bench/results/e-compiler-profile'
    out.mkdir(parents=True, exist_ok=True)
    raw = out / 'linux.jsonl.gz'
    assert not raw.exists(), 'Preserve existing raw; choose a fresh checkout for reruns'
    remote = '/root/minipy-native-e-profile'
    identity = {
        'baseline': run(['git', 'rev-parse', 'HEAD']).strip(),
        'rust': run(['rustc', '+1.96.0', '--version']).strip(),
        'zig': run(['zig', 'version']).strip(),
        'host': run(['uname', '-a']).strip(),
        'target': 'aarch64-unknown-linux-musl',
        'rounds': 3, 'reps_per_process': 60,
        'options': 'opt_level=speed; default verifier enabled; System allocator',
        'vm_initial_state': 'off; started by this task with existing 2-vCPU/2048-MiB vfkit launcher',
        'preflight': run(ssh + ['uname -a; uptime; free -m; df -h /; ps']),
        'source_sha256': {}, 'binaries': {}, 'commands': [],
    }
    files = ['Cargo.toml', 'Cargo.lock', 'src/codegen.rs', 'src/frontend.rs',
             'src/compiler_profile.rs', 'src/lib.rs', 'src/runtime.rs', 'src/ir.rs',
             'examples/compiler_profile.rs', 'tools/profile_compiler.py']
    files += [str(p.relative_to(ROOT)) for p in sorted((ROOT / 'fixtures').glob('*.py'))]
    for file in files:
        identity['source_sha256'][file] = sha(ROOT / file)
    run(ssh + [f'mkdir -p {remote}/fixtures {remote}/ir'])
    for mode in ['plain', 'diagnostic']:
        binary = ROOT / f'target/e-profile-{mode}-linux'
        identity['binaries'][mode] = {'sha256': sha(binary), 'bytes': binary.stat().st_size}
        run(scp + [str(binary), f'{host}:{remote}/{mode}'])
        actual = run(ssh + [f'sha256sum {remote}/{mode}']).split()[0]
        assert actual == sha(binary), 'Remote binary mismatch'
    run(scp + [str(p) for p in sorted((ROOT / 'fixtures').glob('*.py'))] + [f'{host}:{remote}/fixtures/'])
    count = 0
    with gzip.open(raw, 'wt') as f:
        for round_id in range(3):
            modes = ['plain', 'diagnostic'] if round_id % 2 == 0 else ['diagnostic', 'plain']
            cases = CASES if round_id % 2 == 0 else list(reversed(CASES))
            for case in cases:
                for mode in modes:
                    args = [f'{remote}/{mode}', case, '60']
                    if mode == 'diagnostic' and round_id == 0:
                        args += [f'{remote}/ir']
                    command = f'MINIPY_FIXTURES={remote}/fixtures ' + shlex.join(args)
                    identity['commands'].append(command)
                    stdout = run(ssh + [command])
                    rows = [json.loads(line) for line in stdout.splitlines()]
                    compiles = [r for r in rows if r['kind'] == 'compile']
                    assert len(compiles) == 61 and [r['sample'] for r in compiles] == list(range(61))
                    assert all(r['case'] == case and r['diagnostic'] == (mode == 'diagnostic') for r in compiles)
                    # Successful exit also proves untimed actual-machine-code oracle passed.
                    for row in rows:
                        row['round'] = round_id
                        f.write(json.dumps(row, separators=(',', ':')) + '\n')
                        count += 1
                    f.flush()
                    print(round_id, case, mode, '61 compiles + untimed oracle passed', flush=True)
    run(scp + ['-r', f'{host}:{remote}/ir', str(out)])
    identity['rows'] = count
    assert count == 3 * 6 * 2 * 61 + 12
    identity['postflight'] = run(ssh + ['uptime; free -m; df -h /; ps'])
    (out / 'identity.json').write_text(json.dumps(identity, indent=2) + '\n')
    print('Saved', count, 'rows to', raw)


if __name__ == '__main__':
    main()
