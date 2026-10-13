"""Deploy existing builds and serially run the bounded C campaign on an idle VM.
Usage: python3 tools/run_c_linux_v2.py HOST SSH_KEY KNOWN_HOSTS
Build both arms first with prepare_bench_v2_baseline.py and recorded Zig commands.
"""
import json
from pathlib import Path

import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
host, key, known = sys.argv[1:]
ssh = ['ssh', '-i', key, '-o', f'UserKnownHostsFile={known}', '-o', 'StrictHostKeyChecking=yes', '-o', 'BatchMode=yes', host]
scp = ['scp', '-i', key, '-o', f'UserKnownHostsFile={known}', '-o', 'StrictHostKeyChecking=yes']
remote = '/root/minipy-native-c-v2'
source = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
remote_results = f'{remote}/results-{source[:8]}'
output = ROOT / 'bench/results/c-linux-v2'
if output.exists():
    raise SystemExit('use a fresh campaign directory; existing results will not be overwritten')
output.mkdir(parents=True)
commands = []
for name in ['Cargo.toml', 'Cargo.lock', 'src/v2.rs', 'src/v2_common.rs', 'src/memory_v2.rs']:
    assert (ROOT / 'bench' / name).read_bytes() == (ROOT / 'bench/target/c-baseline-source/bench' / name).read_bytes(), name

def run(argv, capture=False):
    commands.append(argv)
    result = subprocess.run(argv, cwd=ROOT, text=True, capture_output=capture, check=False)
    if result.returncode and capture:
        (output / 'deployment-failure.stdout.txt').write_text(result.stdout)
        (output / 'deployment-failure.stderr.txt').write_text(result.stderr)
    result.check_returncode()
    return result.stdout if capture else ''

try:
    preflight = run(ssh + ['uname -a; uptime; free -m; df -h /; ps'], True)
    (output / 'linux-preflight.txt').write_text(preflight)
    print(preflight)
    # Operator has checked the idle preflight. No VM/system configuration changes.
    run(ssh + [f'mkdir -p {remote}/fixtures {remote}/tools {remote}/optimized {remote}/baseline'])
    for label, bindir in [('optimized', ROOT / 'bench/target/aarch64-unknown-linux-musl/release'),
                           ('baseline', ROOT / 'bench/target/c-baseline-source/bench/target/aarch64-unknown-linux-musl/release')]:
        for binary in ['bench-v2', 'memory-v2']:
            run(scp + [str(bindir / binary), f'{host}:{remote}/{label}/{binary}'])
    for path in ['fixtures/weighted_scan.py', 'fixtures/stateful_branch.py', 'tools/run_bench_v2.py', 'tools/summarize_bench_v2.py']:
        run(scp + [str(ROOT / path), f'{host}:{remote}/{path}'])
    command = f'cd {remote} && MINIPY_FIXTURES={remote}/fixtures python3 tools/run_bench_v2.py --profile campaign --rounds 3 --skip-build --bin-dir {remote}/optimized --label optimized --compare-bin-dir {remote}/baseline --compare-label baseline-564381d --out {remote_results}'
    stdout = run(ssh + [command], True)
    (output / 'campaign.stdout.txt').write_text(stdout)
    print(stdout)
    for name in ['v2-raw.jsonl', 'v2-runs.jsonl', 'v2-summary.json', 'report-v2.md']:
        run(scp + [f'{host}:{remote_results}/{name}', str(output / name)])
    # Read-back is independent of the remote runner's success message.
    summary = json.loads((output / 'v2-summary.json').read_text())
    assert summary['commands'] == {'scheduled': 234, 'completed': 234}, summary['commands']
    assert not summary['failures'], summary['failures']
finally:
    (output / 'commands.json').write_text(json.dumps(commands, indent=2) + '\n')
    source = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    identity = {'optimized_source': source, 'deployment_script_source': source, 'baseline_runtime': '564381d304be69d1fd870d4e92530c8024b7c394',
                'identical_bench_source': source,
                'monty': '56949a4c45f51cedc7a96450782f58a8017e114d',
                'rustc_build': subprocess.check_output(['rustup', 'run', '1.96.0', 'rustc', '--version'], text=True).strip(),
                'zig': subprocess.check_output(['zig', 'version'], text=True).strip(),
                'target': 'aarch64-unknown-linux-musl', 'speed_allocator': 'System, no allocation-diagnostics feature',
                'memory_allocator': 'separate diagnostic counting allocator',
                'build_commands': [f'python3 tools/prepare_bench_v2_baseline.py {source}',
                    'cargo +1.96.0 zigbuild --release --locked --manifest-path bench/Cargo.toml --target aarch64-unknown-linux-musl --bin bench-v2 --bin memory-v2',
                    'cargo +1.96.0 zigbuild --release --locked --manifest-path bench/target/c-baseline-source/bench/Cargo.toml --target aarch64-unknown-linux-musl --bin bench-v2 --bin memory-v2'],
                'note': 'Guest deployed source has no .git or rustup; remote campaign_identity null fields are unavailable, not missing build provenance.'}
    (output / 'build-identity.json').write_text(json.dumps(identity, indent=2) + '\n')
