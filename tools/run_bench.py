"""Rebuild and record real local and optional ARM64 Linux benchmark runs.
Local: python3 tools/run_bench.py
With existing idle VM: python3 tools/run_bench.py HOST SSH_KEY KNOWN_HOSTS
Requires rustc 1.96.0, cargo-zigbuild and Zig for the Linux build. Does not install them.
"""
from pathlib import Path
import hashlib
import json
import shlex
import subprocess
import sys

root = Path(__file__).resolve().parents[1]
out = root / 'bench/results'
out.mkdir(parents=True, exist_ok=True)
identity = {'monty': '56949a4c45f51cedc7a96450782f58a8017e114d', 'commands': [], 'source_sha256': {}}
for p in sorted(list((root / 'src').rglob('*.rs')) + list((root / 'fixtures').glob('*.py')) +
                [root / 'Cargo.toml', root / 'Cargo.lock', root / 'bench/Cargo.toml', root / 'bench/Cargo.lock', root / 'bench/src/main.rs']):
    identity['source_sha256'][str(p.relative_to(root))] = hashlib.sha256(p.read_bytes()).hexdigest()
identity['rustc_native_tests'] = subprocess.check_output(['rustc', '--version'], text=True).strip()
identity['rustc_bench'] = subprocess.check_output(['rustup', 'run', '1.96.0', 'rustc', '--version'], text=True).strip()
identity['macos'] = subprocess.check_output(['sw_vers'], text=True).strip()
identity['hardware'] = subprocess.check_output(['sysctl', 'hw.model', 'hw.memsize', 'machdep.cpu.brand_string'], text=True).strip()

def build(target=None):
    cmd = ['cargo', '+1.96.0', 'zigbuild' if target else 'build', '--release', '--manifest-path', 'bench/Cargo.toml']
    if target:
        cmd += ['--target', target]
    identity['commands'].append(' '.join(cmd))
    subprocess.run(cmd, cwd=root, check=True)

build()
for opt in ['speed', 'none']:
    cmd = ['/usr/bin/time', '-l', str(root / 'bench/target/release/quote-bench'), opt]
    identity['commands'].append(f'/usr/bin/time -l bench/target/release/quote-bench {opt}')
    with (out / f'macos-{opt}.csv').open('w') as stdout, (out / f'macos-{opt}-resources.txt').open('w') as stderr:
        subprocess.run(cmd, cwd=root, stdout=stdout, stderr=stderr, check=True)

if len(sys.argv) > 1:
    host, key, known = sys.argv[1:]
    ssh = ['ssh', '-i', key, '-o', f'UserKnownHostsFile={known}', '-o', 'BatchMode=yes']
    scp = ['scp', '-i', key, '-o', f'UserKnownHostsFile={known}']
    # State/capacity is checked before experimentation. The operator must also avoid unrelated VM workloads.
    identity['linux'] = subprocess.check_output(ssh + [host, 'uname -a; free -m; df -h /; ps -eo pid,time,comm'], text=True)
    build('aarch64-unknown-linux-musl')
    remote = '/root/minipy-native-bench'
    subprocess.run(ssh + [host, f'mkdir -p {remote}/fixtures'], check=True)
    subprocess.run(scp + [str(root / 'bench/target/aarch64-unknown-linux-musl/release/quote-bench'), f'{host}:{remote}/'], check=True)
    for p in (root / 'fixtures').glob('*.py'):
        subprocess.run(scp + [str(p), f'{host}:{remote}/fixtures/'], check=True)
    for opt in ['speed', 'none']:
        cmd = f'busybox time -v env MINIPY_FIXTURES={remote}/fixtures {remote}/quote-bench {shlex.quote(opt)}'
        identity['commands'].append('ssh <idle-vm> ' + cmd)
        with (out / f'linux-{opt}.csv').open('w') as stdout, (out / f'linux-{opt}-resources.txt').open('w') as stderr:
            subprocess.run(ssh + [host, cmd], stdout=stdout, stderr=stderr, check=True)
(out / 'identity.json').write_text(json.dumps(identity, indent=2) + '\n')
subprocess.run([sys.executable, 'tools/summarize.py'], cwd=root, check=True)
