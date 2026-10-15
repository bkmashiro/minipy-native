#!/usr/bin/env python3
"""Small fixed-binary resident-request ABBA driver. Stdlib only; see docs/perf-workbench.md."""
import argparse
from contextlib import contextmanager
import fcntl
import gzip
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
SCRATCH = Path.home() / '.hermes/cache/scratch'
LOCK = SCRATCH / 'minipy-perf-build.lock'
DEFAULT_CASES = ('scalar', 'quote', 'ready', 'loopback')
CASES = DEFAULT_CASES + ('scalar_inputs', 'ready_inputs', 'plain_sink', 'escaped_sink', 'joined_sink', 'fstring_sink', 'escaped_join')
BACKENDS = ('tape', 'native')


def sha(path):
    digest = hashlib.sha256()
    with Path(path).open('rb') as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()


def command(args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


@contextmanager
def locked():
    SCRATCH.mkdir(parents=True, exist_ok=True)
    with LOCK.open('a+') as f:
        fcntl.flock(f, fcntl.LOCK_EX)
        yield


def emit(f, row):
    f.write(json.dumps(row, separators=(',', ':')) + '\n')
    f.flush()


def build(args):
    out = args.output.resolve()
    if out.exists() or Path(str(out) + '.json').exists():
        raise ValueError('refuse to overwrite binary/identity')
    if not os.path.ismount('/Volumes/M.2') or not (ROOT / 'target').is_symlink():
        raise ValueError('expected mounted external shared target')
    if (ROOT / 'target').resolve() != Path('/Volumes/M.2/caches/minipy-native-target'):
        raise ValueError('unexpected shared target destination')
    features = 'tool-json' + (',compiler-profile' if args.diagnostic else '')
    cmd = ['cargo', '+1.96.0', 'test', '--offline', '--locked', '--release',
           '--features', features, '--test', 'perf_workbench', '--no-run', '--message-format=json']
    with locked():
        env = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / 'target'), CARGO_BUILD_JOBS='1')
        result = subprocess.run(cmd, cwd=ROOT, env=env, text=True, capture_output=True)
        if result.returncode:
            print(result.stderr, file=sys.stderr)
            for line in result.stdout.splitlines():
                if line.startswith('{'):
                    message = json.loads(line).get('message', {})
                    if message.get('rendered'):
                        print(message['rendered'], file=sys.stderr)
            raise ValueError(f'build failed: {result.returncode}')
        artifacts = [json.loads(s) for s in result.stdout.splitlines() if s.startswith('{')]
        binaries = [r['executable'] for r in artifacts if r.get('reason') == 'compiler-artifact'
                    and r.get('target', {}).get('name') == 'perf_workbench' and r.get('executable')]
        if len(binaries) != 1:
            raise ValueError('expected one test executable')
        out.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(binaries[0], out)
        identity = {'binary_sha256': sha(out), 'diagnostic': args.diagnostic, 'command': cmd,
                    'commit': command(['git', 'rev-parse', 'HEAD']),
                    'rust': command(['rustc', '+1.96.0', '--version']),
                    'host': platform.platform(),
                    'tracked_diff_sha256': hashlib.sha256(command(['git', 'diff', 'HEAD']).encode()).hexdigest(),
                    'sources': {str(p.relative_to(ROOT)): sha(p) for p in
                                sorted((ROOT / 'src').glob('*.rs')) +
                                [ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', ROOT / 'tests/perf_workbench.rs']}}
        Path(str(out) + '.json').write_text(json.dumps(identity, indent=2) + '\n')
    print(json.dumps(identity, indent=2))


def identity(path):
    path = path.resolve()
    meta = json.loads(Path(str(path) + '.json').read_text())
    if meta['binary_sha256'] != sha(path):
        raise ValueError(f'binary changed: {path}')
    return {'path': str(path), **meta}


def validate(rows, reps, diagnostic):
    if len(rows) != reps + 2 or rows[0].get('kind') != 'setup':
        raise ValueError('missing/extra setup or requests')
    if rows[0].get('diagnostic') is not diagnostic:
        raise ValueError('diagnostic identity mismatch')
    if [r.get('sample') for r in rows[1:]] != list(range(reps + 1)):
        raise ValueError('missing/duplicate/out-of-order sample')
    for row in rows:
        if type(row.get('ns')) is not int or row['ns'] <= 0:
            raise ValueError('invalid timing')
    if any(r.get('kind') != 'request' for r in rows[1:]):
        raise ValueError('unexpected row kind')
    if not diagnostic and any('compiler' in r or 'prepare_ns' in r for r in rows[1:]):
        raise ValueError('instrumented samples in formal timing')


def run(args):
    if args.output.exists():
        raise ValueError('refuse to overwrite raw evidence')
    a = identity(args.baseline)
    b = a if args.aa else identity(args.candidate)
    if a['diagnostic'] != args.diagnostic or b['diagnostic'] != args.diagnostic:
        raise ValueError('formal and diagnostic binaries cannot be mixed')
    if a['sources']['tests/perf_workbench.rs'] != b['sources']['tests/perf_workbench.rs']:
        raise ValueError('workload adapter must be identical across arms')
    cells = [(backend, case) for backend in args.backends for case in args.cases]
    with locked():
        # Hash once again under the common build/run lock, and after every subprocess.
        for item in (a, b):
            if sha(item['path']) != item['binary_sha256']:
                raise ValueError('binary changed before run')
        args.output.parent.mkdir(parents=True, exist_ok=True)
        with gzip.open(args.output, 'xt') as f:
            emit(f, {'kind': 'manifest', 'version': 1, 'diagnostic': args.diagnostic,
                     'aa': args.aa, 'arms': {'A': a, 'B': b}, 'reps': args.reps,
                     'rounds': args.rounds, 'cells': cells, 'order': 'ABBA',
                     'entries': {'A': args.baseline_entry,
                                 'B': args.baseline_entry if args.aa else args.candidate_entry},
                     'driver_sha256': sha(__file__), 'host': platform.platform()})
            failures = 0
            for rnd in range(args.rounds):
                for slot, arm in enumerate('ABBA'):
                    for backend, case in cells:
                        item = a if arm == 'A' else b
                        entry = args.baseline_entry if arm == 'A' or args.aa else args.candidate_entry
                        key = {'round': rnd, 'slot': slot, 'arm': arm, 'backend': backend, 'case': case}
                        cmd = [item['path'], '--ignored', '--exact', 'resident_request', '--nocapture', '--test-threads=1']
                        env = dict(os.environ, PW_BACKEND=backend, PW_CASE=case, PW_ENTRY=entry, PW_REPS=str(args.reps))
                        rows = []
                        error = None
                        code = None
                        try:
                            result = subprocess.run(cmd, env=env, capture_output=True, text=True,
                                                    timeout=args.timeout, cwd=ROOT)
                            code = result.returncode
                            # libtest prefixes the first nocapture line with the test name.
                            rows = [json.loads(s.partition('PW ')[2])
                                    for s in result.stdout.splitlines() if 'PW ' in s]
                            if code:
                                raise ValueError(f'exit {code}: {result.stderr[-1500:]} {result.stdout[-1500:]}')
                            validate(rows, args.reps, args.diagnostic)
                            if sha(item['path']) != item['binary_sha256']:
                                raise ValueError('binary changed during run')
                        except (ValueError, OSError, subprocess.TimeoutExpired) as exc:
                            error = str(exc)
                        # Preserve partial rows, but never count a failed process as a timing cell.
                        emit(f, {'kind': 'process', **key, 'ok': error is None, 'error': error,
                                 'exit_code': code, 'expected_samples': args.reps + 1, 'rows': rows})
                        failures += error is not None
                        print(rnd, slot, arm, backend, case, error or 'ok', flush=True)
            emit(f, {'kind': 'end', 'failures': failures})
    summary = reduce(args.output)
    print(json.dumps(summary, indent=2))
    return int(not summary['complete'])


def reduce(path):
    with gzip.open(path, 'rt') as f:
        data = [json.loads(s) for s in f]
    if not data or data[0].get('kind') != 'manifest':
        raise ValueError('missing manifest')
    m = data[0]
    expected = {(r, s, backend, case) for r in range(m['rounds']) for s in range(4)
                for backend, case in m['cells']}
    seen = set()
    failures = []
    groups = {}
    setups = []
    measured = 0
    for p in data[1:]:
        if p['kind'] == 'end':
            continue
        if p['kind'] != 'process':
            raise ValueError('unexpected record')
        key = (p['round'], p['slot'], p['backend'], p['case'])
        if key not in expected or key in seen or p['arm'] != 'ABBA'[p['slot']]:
            raise ValueError('duplicate/unexpected process')
        seen.add(key)
        if not p['ok']:
            failures.append({k: p[k] for k in ('round', 'slot', 'backend', 'case', 'error')})
            continue
        if p.get('exit_code') != 0:
            raise ValueError('successful process has nonzero exit code')
        validate(p['rows'], m['reps'], m['diagnostic'])
        samples = p['rows'][2:]  # setup and sample 0 are NEVER measured requests
        measured += len(samples)
        setups.append(p['rows'][0]['ns'])
        g = groups.setdefault((p['backend'], p['case']), {'A': [], 'B': []})
        process = {'round': p['round'], 'slot': p['slot'],
                   'median_ns': statistics.median(r['ns'] for r in samples),
                   'min_ns': min(r['ns'] for r in samples),
                   'max_ns': max(r['ns'] for r in samples)}
        if m['diagnostic']:
            process['phases_median_ns'] = {
                field: statistics.median(r[field] for r in samples)
                for field in ('prepare_ns', 'execute_including_code_release_ns', 'output_release_ns')}
            process['compiler_median_ns'] = {
                field: statistics.median(r['compiler'][field] for r in samples)
                for field in samples[0]['compiler']}
        g[p['arm']].append(process)
    missing = sorted(expected - seen)
    ended = len([p for p in data if p['kind'] == 'end']) == 1 and data[-1]['kind'] == 'end'
    complete = not failures and not missing and ended
    if ended and data[-1].get('failures') != len(failures):
        raise ValueError('failure count disagrees with end record')
    cells = []
    for (backend, case), arms in sorted(groups.items()):
        cell = {'backend': backend, 'case': case, 'processes': arms}
        # No delta for partial cells or any incomplete campaign.
        if complete:
            medians = {a: statistics.median(p['median_ns'] for p in arms[a]) for a in 'AB'}
            cell.update(medians_ns=medians)
            if not m['diagnostic']:
                cell['b_over_a'] = medians['B'] / medians['A']
        cells.append(cell)
    return {'complete': complete, 'diagnostic': m['diagnostic'], 'aa': m['aa'],
            'expected_processes': len(expected), 'observed_processes': len(seen),
            'expected_measured_samples': len(expected) * m['reps'], 'measured_samples': measured,
            'failures': failures, 'missing_processes': missing, 'end_record': ended,
            'startup_median_ns': statistics.median(setups) if setups else None, 'cells': cells}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='action', required=True)
    b = sub.add_parser('build')
    b.add_argument('--output', type=Path, required=True)
    b.add_argument('--diagnostic', action='store_true')
    r = sub.add_parser('run')
    r.add_argument('--baseline', type=Path, required=True)
    r.add_argument('--candidate', type=Path)
    r.add_argument('--aa', action='store_true')
    r.add_argument('--baseline-entry', choices=('legacy', 'registered'), default='registered')
    r.add_argument('--candidate-entry', choices=('legacy', 'registered'), default='registered')
    r.add_argument('--diagnostic', action='store_true')
    r.add_argument('--cases', choices=CASES, nargs='+', default=list(DEFAULT_CASES))
    r.add_argument('--backends', choices=BACKENDS, nargs='+', default=list(BACKENDS))
    r.add_argument('--rounds', type=int, choices=range(1, 5), default=1)
    r.add_argument('--reps', type=int, choices=range(1, 1001), default=24)
    r.add_argument('--timeout', type=float, default=60)
    r.add_argument('--output', type=Path, required=True)
    s = sub.add_parser('reduce')
    s.add_argument('raw', type=Path)
    args = parser.parse_args()
    if args.action == 'build':
        build(args)
    elif args.action == 'run':
        if not args.aa and args.candidate is None:
            parser.error('--candidate or --aa required')
        if len(set(args.cases)) != len(args.cases) or len(set(args.backends)) != len(args.backends):
            parser.error('duplicate cell selections')
        return run(args)
    else:
        result = reduce(args.raw)
        print(json.dumps(result, indent=2))
        return int(not result['complete'])
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(error, file=sys.stderr)
        sys.exit(1)
