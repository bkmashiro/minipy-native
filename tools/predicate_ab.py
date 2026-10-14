"""F bounded A/B; prebuilt target/f-{baseline,candidate}{,-ir}-linux.
Usage: python3 tools/predicate_ab.py HOST KEY KNOWN_HOSTS [HOST_KEY_ALIAS]
Run only on an inspected idle existing VM. Never overwrites evidence.
"""
import gzip
import hashlib
import json
from pathlib import Path
import shlex
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'bench/results/f-predicate'
CASES = [('scalar', 0), ('weighted', 100), ('weighted', 10000), ('nested', 100), ('nested', 10000)]
BASELINE = '51e83bd2bc6f31d04ba0973b71e702f12da9134e'


def run(args):
    return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    host, key, known, *alias = sys.argv[1:]
    opts = ['-i', key, '-o', f'UserKnownHostsFile={known}', '-o', 'StrictHostKeyChecking=yes', '-o', 'BatchMode=yes']
    if alias:
        opts += ['-o', f'HostKeyAlias={alias[0]}']
    ssh = ['ssh', *opts, host]
    scp = ['scp', *opts]
    OUT.mkdir(parents=True, exist_ok=True)
    raw = OUT / 'linux.jsonl.gz'
    assert not raw.exists(), 'Preserve raw evidence'
    remote = '/root/minipy-native-f-predicate'
    identity = {
        'baseline': BASELINE,
        'checkout': run(['git', 'rev-parse', 'HEAD']).strip(),
        'candidate_patch': 'candidate.patch',
        'rust': run(['rustc', '+1.96.0', '--version']).strip(),
        'zig': run(['zig', 'version']).strip(),
        'target': 'aarch64-unknown-linux-musl',
        'options': 'speed; backtracking; default verifier; System allocator; chunk=256',
        'initial_vm': 'off; existing run-vm.sh started with PTY (non-PTY start rejected before boot)',
        'preflight': run(ssh + ['uname -a; uptime; free -m; df -h /; ps']),
        'source_sha256': {}, 'binaries': {}, 'commands': [],
        'rounds': 3, 'reps': 100, 'summary_samples': '6..100; first=0; transition=1..5',
    }
    files = ['Cargo.toml', 'Cargo.lock', 'src/codegen.rs', 'src/frontend.rs', 'src/ir.rs',
             'src/runtime.rs', 'src/buffer.rs', 'src/provider.rs', 'src/compiler_profile.rs',
             'examples/predicate_ab.rs', 'tools/predicate_ab.py', 'tools/summarize_predicate_ab.py',
             'fixtures/scalar_script.py', 'fixtures/weighted_script.py', 'fixtures/predicate_nested.py']
    identity['source_sha256'] = {p: sha(ROOT / p) for p in files}
    identity['baseline_codegen_sha256'] = hashlib.sha256(subprocess.check_output(['git', 'show', f'{BASELINE}:src/codegen.rs'])).hexdigest()
    patch = run(['git', 'diff', BASELINE, '--', 'src/codegen.rs'])
    assert patch, 'Expected exactly one codegen candidate'
    (OUT / 'candidate.patch').write_text(patch)
    run(ssh + [f'mkdir -p {remote}/ir/baseline {remote}/ir/candidate'])
    for variant in ['baseline', 'candidate']:
        for suffix in ['', '-ir']:
            label = variant + suffix
            binary = ROOT / f'target/f-{label}-linux'
            if not suffix:
                assert 'minipy_native::compiler_profile::' not in run(['nm', '-C', str(binary)])
            identity['binaries'][label] = {'sha256': sha(binary), 'bytes': binary.stat().st_size}
            run(scp + [str(binary), f'{host}:{remote}/{label}'])
            assert run(ssh + [f'sha256sum {remote}/{label}']).split()[0] == sha(binary)
    count = 0
    with gzip.open(raw, 'wt') as f:
        for round_id in range(3):
            cells = CASES if round_id % 2 == 0 else list(reversed(CASES))
            variants = ['baseline', 'candidate'] if round_id % 2 == 0 else ['candidate', 'baseline']
            for case, n in cells:
                for mode in ['cold', 'prepared']:
                    for variant in variants:
                        command = shlex.join([f'{remote}/{variant}', case, mode, str(n), '100'])
                        identity['commands'].append(command)
                        rows = [json.loads(line) for line in run(ssh + [command]).splitlines()]
                        assert len(rows) == 101 and [r['sample'] for r in rows] == list(range(101))
                        for row in rows:
                            assert row['ok'] and row['case'] == case and row['mode'] == mode and row['rows'] == n
                            row.update(round=round_id, variant=variant)
                            f.write(json.dumps(row, separators=(',', ':')) + '\n')
                            count += 1
                        f.flush()
                        print(round_id, case, n, mode, variant, '101 oracles passed', flush=True)
        for variant in ['baseline', 'candidate']:
            for case in ['scalar', 'weighted', 'nested']:
                command = shlex.join([f'{remote}/{variant}-ir', case, 'cold', '100', '1', f'{remote}/ir/{variant}'])
                identity['commands'].append(command)
                rows = [json.loads(line) for line in run(ssh + [command]).splitlines()]
                inventory = [r for r in rows if r['kind'] == 'inventory']
                assert len(inventory) == 2
                for row in inventory:
                    row['variant'] = variant
                    f.write(json.dumps(row, separators=(',', ':')) + '\n')
                    count += 1
    assert count == 6072
    run(scp + ['-r', f'{host}:{remote}/ir', str(OUT)])
    identity['postflight'] = run(ssh + ['uptime; free -m; df -h /; ps'])
    identity['rows'] = count
    (OUT / 'identity.json').write_text(json.dumps(identity, indent=2) + '\n')


if __name__ == '__main__':
    main()
