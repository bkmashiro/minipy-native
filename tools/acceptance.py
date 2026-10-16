#!/usr/bin/env python3
"""Fixed complete-script acceptance on native and tape. No performance sampling."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shlex
import subprocess

from perf_workbench import ROOT, locked, sha

# Existing full scripts, payload variants and lifecycle assertions stay at source.
# Each sentinel protects a named part of the integrated contract against an empty
# or accidentally filtered suite. All tests in these seven suites must run.
SUITES = {
    'di_corpus': 'controlled_smoke_both_backends_and_separate_memory_mode',
    'json_get': 'unchanged_cv05_and_complete_tool_script',
    'json_fstring': 'unchanged_cv01_and_unexecuted_branch',
    'json_array': 'cv04_original_and_ordered_lazy_fields',
    'line_scan': 'complete_count_and_filter_both_backends',
    'cy_routing': 'cancellation_after_submission_keeps_effect_once_and_never_routes',
    'dl_contract': 'uniform_stdout_policy_counts_utf8_and_keeps_failed_print_atomic',
}
LINUX_TARGET = 'aarch64-unknown-linux-musl'


def listed_tests(text, sentinel):
    names = re.findall(r'^([A-Za-z0-9_:]+): test$', text, re.M)
    counts = re.findall(r'^(\d+) tests?, (\d+) benchmarks?$', text, re.M)
    if (sentinel not in names or len(names) != len(set(names))
            or counts != [(str(len(names)), '0')]):
        raise ValueError('missing sentinel, duplicate tests or invalid test inventory')
    return names


def passed_tests(text, count):
    results = re.findall(
        r'^test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored; '
        r'(\d+) measured; (\d+) filtered out;', text, re.M)
    if results != [('ok', str(count), '0', '0', '0', '0')]:
        raise ValueError('acceptance requires every inventoried test to pass without skips')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True, help='new evidence directory')
    parser.add_argument('--linux-host', help='existing authorized ARM64 Linux SSH endpoint')
    parser.add_argument('--ssh-option', action='append', default=[],
                        help='one ssh/scp option, e.g. --ssh-option=-i --ssh-option=/path/key')
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    report = {'complete': False, 'suites': SUITES, 'commands': [], 'runs': []}

    def save():
        (out / 'checks.json').write_text(json.dumps(report, indent=2) + '\n')

    def run(cmd, name, timeout=600, env=None):
        result = subprocess.run(cmd, cwd=ROOT, env=env, capture_output=True,
                                text=True, timeout=timeout)
        (out / (name + '.stdout.txt')).write_text(result.stdout)
        (out / (name + '.stderr.txt')).write_text(result.stderr)
        report['commands'].append({'command': cmd, 'name': name, 'exit_code': result.returncode})
        save()
        if result.returncode:
            raise RuntimeError(f'{name}: exit {result.returncode}; see {out}')
        return result.stdout

    try:
        target = ROOT / 'target'
        if platform.system() == 'Darwin' and (
                not os.path.ismount('/Volumes/M.2') or not target.is_symlink()
                or target.resolve() != Path('/Volumes/M.2/caches/minipy-native-target')):
            raise RuntimeError('existing mounted external shared target required')
        if not target.is_dir():
            raise RuntimeError('reuse an existing target; do not create another build tree')
        if os.environ.get('RUSTFLAGS') or os.environ.get('CARGO_ENCODED_RUSTFLAGS'):
            raise RuntimeError('ordinary release required; unexpected compiler flags')
        report.update(head=run(['git', 'rev-parse', 'HEAD'], 'head').strip(),
                      platform=platform.platform(), target=str(target.resolve()))
        paths = [ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', Path(__file__).resolve(),
                 ROOT / 'examples/di_corpus.rs', ROOT / 'tools/perf_workbench.py',
                 *sorted((ROOT / 'src').rglob('*.rs')),
                 *(ROOT / f'tests/{name}.rs' for name in SUITES),
                 *sorted((ROOT / 'fixtures').rglob('*.py')),
                 ROOT / 'fixtures/di/smoke.jsonl',
                 ROOT / 'bench/results/dr/splitlines-oracle.json']
        report['sources'] = {str(p.relative_to(ROOT)): sha(p) for p in paths}
        report['rustc'] = run(['rustc', '+1.96.0', '-Vv'], 'rustc')
        ssh = ['ssh', '-o', 'BatchMode=yes', '-o', 'ConnectTimeout=10',
               '-o', 'StrictHostKeyChecking=yes', *args.ssh_option]
        scp = ['scp', '-O', '-o', 'BatchMode=yes', '-o', 'StrictHostKeyChecking=yes',
               *args.ssh_option]
        if args.linux_host:
            info = run([*ssh, args.linux_host,
                        'uname -sm; uptime; free -m; df -h /; ps -o pid,comm'], 'linux-preflight')
            if info.splitlines()[0] != 'Linux aarch64':
                raise RuntimeError('Linux endpoint must match aarch64-unknown-linux-musl')
            report['zig'] = run(['zig', 'version'], 'zig').strip()
        env = dict(os.environ, CARGO_TARGET_DIR=str(target), CARGO_BUILD_JOBS='1',
                   RUSTUP_AUTO_INSTALL='0')
        # Serialize the entire build/deploy/run against existing research work.
        with locked():
            for lane in ['host'] + (['linux'] if args.linux_host else []):
                cmd = ['cargo', '+1.96.0', 'test' if lane == 'host' else 'zigbuild',
                       '--offline', '--locked', '--release', '--features', 'tool-json',
                       '--message-format=json']
                for suite in SUITES:
                    cmd += ['--test', suite]
                cmd += ['--no-run'] if lane == 'host' else ['--target', LINUX_TARGET]
                raw = run(cmd, lane + '-build', timeout=1800, env=env)
                artifacts = [json.loads(s) for s in raw.splitlines() if s.startswith('{')]
                binaries = {r['target']['name']: Path(r['executable']) for r in artifacts
                            if r.get('reason') == 'compiler-artifact' and r.get('executable')
                            and r['target']['name'] in SUITES}
                if set(binaries) != set(SUITES):
                    raise RuntimeError('missing acceptance executables')
                remote = '/root/minipy-acceptance-' + hashlib.sha256(str(out).encode()).hexdigest()[:12]
                if lane == 'linux':
                    run([*ssh, args.linux_host, 'mkdir ' + shlex.quote(remote)], 'linux-mkdir')
                for suite, binary in sorted(binaries.items()):
                    digest = sha(binary)
                    entry = [str(binary)]
                    remote_binary = remote + '/' + suite
                    if lane == 'linux':
                        run([*scp, str(binary), args.linux_host + ':' + remote_binary], suite + '-upload')
                        check = run([*ssh, args.linux_host, 'sha256sum ' + remote_binary], suite + '-remote-sha')
                        if check.split()[0] != digest:
                            raise RuntimeError('deployed binary hash mismatch')
                        entry = [*ssh, args.linux_host, remote_binary]
                    names = listed_tests(run([*entry, '--list'], lane + '-' + suite + '-list'), SUITES[suite])
                    text = run([*entry, '--test-threads=1'], lane + '-' + suite + '-run')
                    passed_tests(text, len(names))
                    if sha(binary) != digest:
                        raise RuntimeError('binary changed during acceptance')
                    if lane == 'linux':
                        check = run([*ssh, args.linux_host, 'sha256sum ' + remote_binary], suite + '-remote-sha-after')
                        if check.split()[0] != digest:
                            raise RuntimeError('remote binary changed during acceptance')
                    report['runs'].append({'lane': lane, 'suite': suite, 'passed': len(names),
                                           'tests': names, 'binary_sha256': digest})
                    save()
                    print(f'{lane}: {suite}: {len(names)} passed', flush=True)
                if lane == 'linux':
                    # Only our seven known uploaded files, then the empty directory.
                    cleanup = 'rm ' + ' '.join(remote + '/' + s for s in SUITES) + '; rmdir ' + remote
                    run([*ssh, args.linux_host, cleanup], 'linux-cleanup')
        if any(sha(ROOT / p) != digest for p, digest in report['sources'].items()):
            raise RuntimeError('source changed during acceptance')
        report['complete'] = True
        report['passed'] = {lane: sum(r['passed'] for r in report['runs'] if r['lane'] == lane)
                            for lane in sorted({r['lane'] for r in report['runs']})}
    except Exception as error:
        report['error'] = str(error)
        raise
    finally:
        save()
    print(json.dumps(report['passed']))


if __name__ == '__main__':
    main()
