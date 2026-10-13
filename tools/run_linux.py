"""Build tests on macOS with Zig, then actually execute them on an idle ARM64 Linux VM.
Usage: python3 tools/run_linux.py HOST SSH_KEY KNOWN_HOSTS
No guest toolchain install or system changes. Copies only this project's test binaries.
"""
import json
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[1]
host, key, known_hosts = sys.argv[1:]
ssh = ['ssh', '-i', key, '-o', f'UserKnownHostsFile={known_hosts}', '-o', 'BatchMode=yes']
scp = ['scp', '-i', key, '-o', f'UserKnownHostsFile={known_hosts}']
cmd = ['cargo', 'zigbuild', '--tests', '--release', '--target', 'aarch64-unknown-linux-musl', '--message-format=json']
build = subprocess.run(cmd, cwd=root, capture_output=True, text=True, check=True)
artifacts = []
for line in build.stdout.splitlines():
    data = json.loads(line)
    if data.get('reason') == 'compiler-artifact' and data.get('executable') and data.get('profile', {}).get('test') and data['target']['kind'] != ['bin']:
        artifacts.append(data['executable'])
assert len(artifacts) == 3, artifacts
remote = '/root/minipy-native-tests'
subprocess.run(ssh + [host, f'mkdir -p {remote}'], check=True)
log = 'command: ' + ' '.join(cmd) + '\n' + build.stderr
log += subprocess.check_output(['rustc', '--version'], text=True)
log += subprocess.check_output(['zig', 'version'], text=True)
log += subprocess.check_output(ssh + [host, 'uname -a; free -m; df -h /'], text=True)
for binary in artifacts:
    name = Path(binary).name
    subprocess.run(scp + [binary, f'{host}:{remote}/{name}'], check=True)
    result = subprocess.run(ssh + [host, f'{remote}/{name} --test-threads=1'], capture_output=True, text=True)
    log += f'\nExecuted {name}, exit={result.returncode}\n' + result.stdout + result.stderr
    print(result.stdout, end='')
    if result.returncode:
        (root / 'bench/results').mkdir(parents=True, exist_ok=True)
        (root / 'bench/results/linux-tests.txt').write_text(log)
        raise SystemExit(result.returncode)
(root / 'bench/results').mkdir(parents=True, exist_ok=True)
(root / 'bench/results/linux-tests.txt').write_text(log.rstrip() + '\n')
print('Linux execution evidence:', root / 'bench/results/linux-tests.txt')
