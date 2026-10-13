"""Bounded generic helper rerun: local or HOST SSH_KEY KNOWN_HOSTS.
Retains historical campaign raw files and identity; no guest/global setup.
"""
from pathlib import Path
import hashlib
import json
import subprocess
import sys

root = Path(__file__).resolve().parents[1]
out = root / 'bench/results'
identity = {'parent_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip(),
            'monty': '56949a4c45f51cedc7a96450782f58a8017e114d', 'commands': [], 'source_sha256': {},
            'stateful_branch': 'unsupported by the fixed stateless helper'}
paths = list((root / 'src').rglob('*.rs')) + list((root / 'bench/src').glob('*.rs')) + list((root / 'fixtures').glob('*.py'))
paths += [root / p for p in ['Cargo.toml', 'Cargo.lock', 'bench/Cargo.toml', 'bench/Cargo.lock', 'tools/run_generic.py', 'tools/summarize_generic.py']]
for p in sorted(paths):
    identity['source_sha256'][str(p.relative_to(root))] = hashlib.sha256(p.read_bytes()).hexdigest()
identity['rustc'] = subprocess.check_output(['rustup', 'run', '1.96.0', 'rustc', '--version'], text=True).strip()
identity['macos'] = subprocess.check_output(['sw_vers'], text=True).strip()
identity['hardware'] = subprocess.check_output(['sysctl', 'hw.model', 'hw.memsize', 'machdep.cpu.brand_string'], text=True).strip()

def build(target=None):
    cmd = ['cargo', '+1.96.0', 'zigbuild' if target else 'build', '--release', '--manifest-path', 'bench/Cargo.toml', '--locked']
    if target:
        cmd += ['--target', target]
    identity['commands'].append(' '.join(cmd))
    subprocess.run(cmd, cwd=root, check=True)

build()
identity['commands'].append('/usr/bin/time -l bench/target/release/quote-bench generic')
with (out / 'generic-macos.csv').open('w') as stdout, (out / 'generic-macos-resources.txt').open('w') as stderr:
    subprocess.run(['/usr/bin/time', '-l', str(root / 'bench/target/release/quote-bench'), 'generic'], cwd=root, stdout=stdout, stderr=stderr, check=True)
if len(sys.argv) > 1:
    host, key, known = sys.argv[1:]
    ssh = ['ssh', '-i', key, '-o', f'UserKnownHostsFile={known}', '-o', 'BatchMode=yes']
    scp = ['scp', '-i', key, '-o', f'UserKnownHostsFile={known}']
    identity['linux'] = subprocess.check_output(ssh + [host, 'uname -a; free -m; df -h /; ps -eo pid,time,comm'], text=True)
    print(identity['linux'])
    build('aarch64-unknown-linux-musl')
    remote = '/root/minipy-native-generic'
    subprocess.run(ssh + [host, f'mkdir -p {remote}/fixtures'], check=True)
    subprocess.run(scp + [str(root / 'bench/target/aarch64-unknown-linux-musl/release/quote-bench'), f'{host}:{remote}/'], check=True)
    subprocess.run(scp + [str(root / 'fixtures/weighted_scan.py'), f'{host}:{remote}/fixtures/'], check=True)
    cmd = f'busybox time -v env MINIPY_FIXTURES={remote}/fixtures {remote}/quote-bench generic'
    identity['commands'].append('ssh <idle-vm> ' + cmd)
    with (out / 'generic-linux.csv').open('w') as stdout, (out / 'generic-linux-resources.txt').open('w') as stderr:
        subprocess.run(ssh + [host, cmd], stdout=stdout, stderr=stderr, check=True)
(out / 'generic-identity.json').write_text(json.dumps(identity, indent=2) + '\n')
subprocess.run([sys.executable, 'tools/summarize_generic.py'], cwd=root, check=True)
