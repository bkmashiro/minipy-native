#!/usr/bin/env python3
"""CN: one bounded Rust PGO candidate; identical BT adapters, shared target/lock."""
import argparse
import gzip
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

from perf_workbench import ROOT, SCRATCH, locked, sha, validate

OUT = ROOT / 'evidence/cn-pgo'
WORK = SCRATCH / 'cn-pgo'
TARGET = 'aarch64-apple-darwin'
TRAIN = ('scalar', 'quote', 'ready', 'plain_sink', 'loopback')
HOLDOUT = ('branch', 'scalar_inputs', 'madd_loop', 'escaped_join', 'detach_wait')
REPS = 12
RECORDS = []
ENV = dict(os.environ, RUSTUP_AUTO_INSTALL='0', CARGO_BUILD_JOBS='1',
           CARGO_TARGET_DIR=str(ROOT / 'target'))


def run(cmd, env=None):
    start = time.monotonic_ns()
    r = subprocess.run([str(x) for x in cmd], cwd=ROOT, env=env or ENV,
                       text=True, capture_output=True)
    elapsed = time.monotonic_ns() - start
    row = dict(command=[str(x) for x in cmd], exit_code=r.returncode,
               wall_ns=elapsed, stdout=r.stdout, stderr=r.stderr,
               experiment_environment={k: v for k, v in (env or ENV).items()
                                       if k.startswith('PW_') or k in
                                       ('RUSTFLAGS', 'LLVM_PROFILE_FILE', 'CARGO_TARGET_DIR',
                                        'CARGO_BUILD_JOBS', 'RUSTUP_AUTO_INSTALL')})
    RECORDS.append(row)
    print('exit', r.returncode, 'seconds', round(elapsed/1e9, 2), *cmd, flush=True)
    if r.returncode:
        print(r.stdout[-3000:], r.stderr[-5000:], flush=True)
    # Preserve a failure before throwing.
    with gzip.open(OUT / 'execution.json.gz', 'wt') as f:
        json.dump(RECORDS, f, indent=2)
    r.check_returncode()
    return r.stdout


def sources():
    return {str(p.relative_to(ROOT)): sha(p) for p in
            sorted((ROOT / 'src').rglob('*.rs')) +
            [ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', ROOT / 'tests/perf_workbench.rs']}


def build(name, flags):
    env = dict(ENV, RUSTFLAGS=flags)
    cmd = ['cargo', '+1.96.0', 'test', '--offline', '--locked', '--release',
           '--target', TARGET, '--features', 'tool-json', '--test', 'perf_workbench',
           '--no-run', '--message-format=json']
    text = run(cmd, env)
    artifacts = [json.loads(s) for s in text.splitlines() if s.startswith('{')]
    bins = [r['executable'] for r in artifacts if r.get('executable') and
            r.get('target', {}).get('name') == 'perf_workbench']
    assert len(bins) == 1
    dest = WORK / name
    shutil.copy2(bins[0], dest)
    meta = dict(binary_sha256=sha(dest), binary_bytes=dest.stat().st_size,
                diagnostic=False, command=cmd, rustflags=flags, target=TARGET,
                commit=run(['git', 'rev-parse', 'HEAD']).strip(), sources=sources(),
                rust=run(['rustc', '+1.96.0', '-vV']),
                build_wall_ns=RECORDS[-3]['wall_ns'])
    Path(str(dest) + '.json').write_text(json.dumps(meta, indent=2) + '\n')
    (OUT / (name + '-identity.json')).write_text(json.dumps(meta, indent=2) + '\n')
    return dest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['collect', 'inspect'])
    args = parser.parse_args()
    if args.action == 'inspect':
        inspect()
        return
    assert os.path.ismount('/Volumes/M.2')
    assert (ROOT / 'target').resolve() == Path('/Volumes/M.2/caches/minipy-native-target')
    assert not WORK.exists(), 'refuse repeat experiment'
    OUT.mkdir(parents=True, exist_ok=True)
    WORK.mkdir()
    profile = WORK / 'profiles'
    profile.mkdir()
    base = run(['git', 'rev-parse', 'HEAD']).strip()
    before = sources()
    with locked():
        sysroot = Path(run(['rustc', '+1.96.0', '--print', 'sysroot']).strip())
        profdata = sysroot / 'lib/rustlib' / TARGET / 'bin/llvm-profdata'
        version = run([profdata, '--version'])
        assert '22.1.2' in version
        a = build('baseline', '')
        instrumented = build('instrumented', '-Cprofile-generate=' + str(profile))
        for backend in ('tape', 'native'):
            for case in TRAIN:
                env = dict(ENV, PW_BACKEND=backend, PW_CASE=case, PW_ENTRY='registered',
                           PW_REPS='24', LLVM_PROFILE_FILE=str(profile / '%m-%p.profraw'))
                text = run([instrumented, '--ignored', '--exact', 'resident_request',
                            '--nocapture', '--test-threads=1'], env)
                rows = [json.loads(s.partition('PW ')[2]) for s in text.splitlines() if 'PW ' in s]
                validate(rows, 24, False)
        files = sorted(profile.glob('*.profraw'))
        assert files
        merged = WORK / 'merged.profdata'
        run([profdata, 'merge', '-o', merged, *files])
        run([profdata, 'show', '--all-functions', '--counts', merged])
        flags = '-Cprofile-use=' + str(merged) + ' -Cllvm-args=-pgo-warn-missing-function'
        b = build('candidate', flags)
        (OUT / 'profile.json').write_text(json.dumps(dict(
            base=base, training_cases=TRAIN, holdout_cases=HOLDOUT,
            training_reps=24, holdout_reps=REPS, target=TARGET,
            profile_sha256=sha(merged), profile_bytes=merged.stat().st_size,
            profraw=[dict(name=p.name, bytes=p.stat().st_size, sha256=sha(p)) for p in files],
            llvm_profdata_sha256=sha(profdata), llvm_profdata_version=version,
            sources=before), indent=2) + '\n')
        # B correctness: all production configurations. Instrumentation never timed.
        for features in (None, 'tool-json,debug-runtime'):
            cmd = ['cargo', '+1.96.0', 'test', '--release', '--offline', '--locked', '--target', TARGET]
            if features:
                cmd += ['--features', features]
            run(cmd, dict(ENV, RUSTFLAGS=flags))
        assert sources() == before
    # Driver acquires the same lock itself. No build may alter copied binaries.
    for aa in (False, True):
        cmd = [sys.executable, 'tools/perf_workbench.py', 'run', '--baseline', a,
               '--cases', *HOLDOUT, '--rounds', '1', '--reps', str(REPS),
               '--output', OUT / ('aa.jsonl.gz' if aa else 'ab.jsonl.gz')]
        cmd += ['--aa'] if aa else ['--candidate', b]
        run(cmd)
    run([sys.executable, '-m', 'unittest', 'discover', '-s', 'tools', '-p', 'test_perf_workbench.py'])
    run(['cargo', '+stable', 'fmt', '--all', '--', '--check'])
    run(['git', 'diff', '--check'])
    assert sources() == before
    print('complete; no automatic acceptance, tuning, or production flags', flush=True)


def inspect():
    """Post-run resource and instrumentation inspection; no new timings/builds."""
    def read(cmd):
        return subprocess.check_output([str(s) for s in cmd], cwd=ROOT, text=True)
    with locked():
        sysroot = Path(read(['rustc', '+1.96.0', '--print', 'sysroot']).strip())
        tools = sysroot / 'lib/rustlib' / TARGET / 'bin'
        manifest = next((sysroot / 'lib/rustlib').glob('manifest-llvm-tools*'))
        files = [sysroot / s.split(':', 1)[1] for s in manifest.read_text().splitlines()
                 if s.startswith('file:')]
        machines = {}
        for name in ('baseline', 'instrumented', 'candidate'):
            binary = WORK / name
            symbols = read([tools / 'llvm-nm', '--defined-only', binary])
            matches = [s for s in symbols.splitlines() if '__llvm_profile' in s]
            assert bool(matches) == (name == 'instrumented')
            machines[name] = dict(binary_sha256=sha(binary), profile_symbols=matches,
                                  llvm_size=read([tools / 'llvm-size', binary]))
        with gzip.open(OUT / 'execution.json.gz', 'rt') as f:
            records = json.load(f)
        artifact_paths = set()
        for row in records:
            for line in row['stdout'].splitlines():
                if line.startswith('{'):
                    try:
                        item = json.loads(line)
                    except ValueError:
                        continue
                    if item.get('reason') == 'compiler-artifact':
                        artifact_paths.update(item.get('filenames', []))
        artifacts = [Path(p) for p in artifact_paths if Path(p).exists()]
        result = dict(machines=machines, component=dict(
            official_compressed_bytes=47464228, files=len(files),
            logical_bytes=sum(p.stat().st_size for p in files),
            allocated_bytes=sum(p.stat().st_blocks * 512 for p in files)),
            cargo_json_declared_existing_artifacts=dict(
                count=len(artifacts), logical_bytes=sum(p.stat().st_size for p in artifacts),
                allocated_bytes=sum(p.stat().st_blocks * 512 for p in artifacts)),
            du_kib=read(['du', '-sk', str(ROOT / 'target') + '/', str(WORK)]),
            scope='Declared artifacts include shared existing deps, omit unreported objects/test outputs; not a net disk delta. Initial shared target was rounded 23 GiB.')
        path = OUT / 'inspection.json'
        assert not path.exists(), 'refuse overwrite inspection'
        path.write_text(json.dumps(result, indent=2) + '\n')
        print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
