"""Small serial campaign. Run on the measured host; no VM/CI setup or digests.
Default speed build has no allocation instrumentation. Memory is a separate binary.
A comparison directory must contain THIS harness built against the other runtime.
"""
import argparse
import json
import platform
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def plan(profile):
    smoke = profile == 'smoke'
    cases = []
    for case in ['weighted_scan', 'stateful_branch']:
        for rows in ([100] if smoke else [0, 100, 10_000]):
            cases.append(('bench-v2', ['latency', '--case', case, '--rows', str(rows), '--repeats', '2' if smoke else '7']))
    for concurrency in ([2] if smoke else [1, 8, 32]):
        for delay in ([0] if smoke else [0, 1000]):
            cases.append(('bench-v2', ['concurrent', '--rows', '10000', '--requests', str(concurrency*4), '--concurrency', str(concurrency), '--mixed', 'true', '--delay-us', str(delay)]))
    for engine in ['native', 'monty']:
        for tasks in ([0, 4] if smoke else [0, 1, 8, 32]):
            cases.append(('memory-v2', ['--engine', engine, '--tasks', str(tasks), '--rows', '0', '--code', 'shared']))
        n = '4' if smoke else '8'
        for code, cancel in [('shared', 'false'), ('unique', 'false'), ('shared', 'true')]:
            cases.append(('memory-v2', ['--engine', engine, '--tasks', n, '--rows', '1000' if smoke else '10000', '--code', code, '--cancel', cancel]))
        # 10B is payload. The zero-length control isolates fixed representation cost.
        for tasks, size in ([(4, 0), (4, 10)] if smoke else [(0, 10), (8, 10), (32, 10), (8, 0), (8, 4096)]):
            cases.append(('memory-v2', ['--engine', engine, '--tasks', str(tasks), '--bytes', str(size), '--code', 'shared']))
    if not smoke:
        for delay in [0, 1000]:
            cases.append(('bench-v2', ['cold-arrival', '--rows', '10000', '--requests', '64', '--concurrency', '8', '--mixed', 'true', '--arrival-us', '1000', '--delay-us', str(delay)]))
        cases.append(('memory-v2', ['--engine', 'monty', '--tasks', '8', '--rows', '10000', '--code', 'shared', '--clear-quotes', 'true']))
    return cases


def identity(root):
    def command(args):
        r = subprocess.run(args, cwd=root, text=True, capture_output=True, check=False)
        return r.stdout.strip() if r.returncode == 0 else None
    return {'kind': 'campaign_identity', 'host': platform.platform(), 'machine': platform.machine(),
            'source_commit': command(['git', 'rev-parse', 'HEAD']),
            'source_dirty': command(['git', 'status', '--porcelain']),
            'toolchain': command(['rustup', 'run', '1.96.0', 'rustc', '--version']),
            'monty': '56949a4c45f51cedc7a96450782f58a8017e114d'}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--profile', choices=['smoke', 'campaign'], default='smoke')
    p.add_argument('--out', type=Path, required=True)
    p.add_argument('--bin-dir', type=Path, default=ROOT / 'bench/target/release')
    p.add_argument('--compare-bin-dir', type=Path)
    p.add_argument('--label', default='current')
    p.add_argument('--compare-label', default='baseline-564381d')
    p.add_argument('--skip-build', action='store_true')
    p.add_argument('--rounds', type=int, default=1)
    p.add_argument('--timeout', type=float, default=60)
    args = p.parse_args()
    if not 1 <= args.rounds <= 20 or args.timeout <= 0:
        p.error('rounds 1..20, timeout >0')
    args.out.mkdir(parents=True, exist_ok=True)
    if (args.out / 'v2-raw.jsonl').exists():
        p.error('output already contains raw data; choose a fresh directory')
    if not args.skip_build:
        subprocess.run(['cargo', '+1.96.0', 'build', '--release', '--locked', '--manifest-path', 'bench/Cargo.toml', '--bin', 'bench-v2', '--bin', 'memory-v2'], cwd=ROOT, check=True)
    arms = [(args.label, args.bin_dir.resolve())]
    if args.compare_bin_dir:
        arms.append((args.compare_label, args.compare_bin_dir.resolve()))
    ident = identity(ROOT)
    ident.update({'profile': args.profile, 'rounds': args.rounds, 'scheduled_commands': len(plan(args.profile))*args.rounds*len(arms), 'arms': [(name, str(path)) for name, path in arms],
                  'comparison_note': 'caller must build the identical harness against each source; labels are not binary provenance'})
    failures = 0
    with (args.out / 'v2-raw.jsonl').open('w') as raw, (args.out / 'v2-runs.jsonl').open('w') as runs:
        raw.write(json.dumps(ident) + '\n')
        index = 0
        for run in range(args.rounds):
            for binary, cell in plan(args.profile):
                order = arms if run % 2 == 0 else arms[::-1]
                for label, bindir in order:
                    cmd = [str(bindir / binary)] + cell
                    if binary == 'bench-v2':
                        cmd += ['--sample', str(run)]
                    status = 'completed'
                    try:
                        result = subprocess.run(cmd, cwd=ROOT, text=True, capture_output=True, timeout=args.timeout, check=False)
                        stdout, stderr, code = result.stdout, result.stderr, result.returncode
                        if code:
                            status = 'failed'
                    except subprocess.TimeoutExpired as e:
                        stdout = e.stdout or ''
                        stderr = e.stderr or ''
                        if isinstance(stdout, bytes):
                            stdout = stdout.decode(errors='replace')
                        if isinstance(stderr, bytes):
                            stderr = stderr.decode(errors='replace')
                        code, status = None, 'timeout'
                    except OSError as e:
                        stdout, stderr, code, status = '', str(e), None, 'failed'
                    count = 0
                    for line in stdout.splitlines():
                        try:
                            row = json.loads(line)
                        except json.JSONDecodeError:
                            status = 'failed'
                            stderr += '\ninvalid JSON stdout: ' + line
                            continue
                        row.update({'arm': label, 'independent_run': run, 'command_index': index})
                        if row.get('kind') == 'failure':
                            status = 'failed'
                        raw.write(json.dumps(row) + '\n')
                        count += 1
                    if count == 0:
                        status = 'failed' if status == 'completed' else status
                    entry = {'command_index': index, 'arm': label, 'independent_run': run, 'argv': cmd, 'status': status, 'returncode': code, 'raw_rows': count}
                    runs.write(json.dumps(entry) + '\n')
                    if status != 'completed':
                        failures += 1
                        raw.write(json.dumps({'kind': 'command_failure', **entry, 'stderr': stderr}) + '\n')
                    if stderr:
                        (args.out / f'command-{index}.stderr.txt').write_text(stderr)
                    raw.flush()
                    runs.flush()
                    print(f'{index}: {label} {binary} {status}', flush=True)
                    index += 1
    subprocess.run([sys.executable, str(ROOT / 'tools/summarize_bench_v2.py'), str(args.out)], check=True)
    return 1 if failures else 0


if __name__ == '__main__':
    sys.exit(main())
