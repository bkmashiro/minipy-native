"""K: one bounded interpreter candidate, existing uninstrumented harness.
Usage: final_hotspot_ab.py HOST KEY KNOWN_HOSTS BASE CANDIDATE
Four serial AB/BA/BA/AB pairs; mixed windows and shapes both paired.
No final four-engine campaign, allocator, or diagnostic counters in speed.
"""
import gzip
import hashlib
import json
from pathlib import Path
import shlex
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'bench/results/k-final-hotspot'
SHAPES = [('scalar', 0), ('weighted', 100), ('stateful', 100),
          ('weighted', 10000), ('stateful', 100000)]


def run(args):
    return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    host, key, known, base, candidate = sys.argv[1:]
    opts = ['-i', key, '-o', f'UserKnownHostsFile={known}',
            '-o', 'StrictHostKeyChecking=yes', '-o', 'BatchMode=yes']
    ssh = ['ssh', *opts, host]
    remote = '/root/minipy-native-k'
    OUT.mkdir(parents=True, exist_ok=True)
    raw = OUT / 'next-header.jsonl.gz'
    assert not raw.exists(), 'preserve raw'
    identity = {
        'head': run(['git', 'rev-parse', 'HEAD']).strip(),
        'patch': run(['git', 'diff', '--binary']),
        'rust': run(['rustc', '+1.96.0', '--version']).strip(),
        'zig': run(['zig', 'version']).strip(),
        'preflight': run(ssh + ['uname -a; uptime; free -m; df -h /; ps']),
        'binaries': {}, 'commands': [],
        'order': ['AB', 'BA', 'BA', 'AB'],
        'accounting': 'J: mixed 9 windows keep sample>=2; shapes 41 keep>=11; '
                      'chunk256; ready and two real 1000us Tool sleeps; '
                      'cold includes parse/prepare/execute/reclaim, warm explicit reuse',
    }
    identity['source_sha256'] = {
        str(p.relative_to(ROOT)): sha(p)
        for pattern in ['src/*.rs', 'fixtures/*.py', 'tests/interpreter.rs',
                        'bench/src/bin/interpreter_ab.rs', 'bench/src/v2_common.rs',
                        'tools/final_hotspot_ab.py', 'tools/summarize_final_hotspot.py',
                        'Cargo*', 'bench/Cargo*']
        for p in ROOT.glob(pattern) if p.is_file()
    }
    run(ssh + [f'mkdir -p {remote}'])
    for build, path in [('base', base), ('candidate', candidate)]:
        p = Path(path)
        run(['scp', *opts, str(p), f'{host}:{remote}/{build}'])
        assert run(ssh + [f'sha256sum {remote}/{build}']).split()[0] == sha(p)
        identity['binaries'][build] = {'sha256': sha(p), 'bytes': p.stat().st_size}
    count = 0
    with gzip.open(raw, 'wt') as f:
        def collect(build, case, mode, n, reps, pair, delay):
            nonlocal count
            cmd = 'MINIPY_CHUNK=256 MINIPY_TOOL_DELAY_US=' + str(delay) + ' ' + shlex.join(
                [f'{remote}/{build}', 'interpreter', case, mode, str(n), str(reps)])
            identity['commands'].append(cmd)
            rows = [json.loads(line) for line in run(ssh + [cmd]).splitlines()]
            assert len(rows) == reps * (40 if mode.startswith('mixed-') else 1)
            assert all(r['completed'] for r in rows)
            for r in rows:
                r.update(build=build, round=pair, chunk=256)
                f.write(json.dumps(r, separators=(',', ':')) + '\n')
            count += len(rows)
            f.flush()
        for pair in range(4):
            order = ['base', 'candidate'] if pair in [0, 3] else ['candidate', 'base']
            for mix in ['light', 'scan']:
                for mode, delay in [('mixed-cold', 0), ('mixed-cold', 1000), ('mixed-warm', 0)]:
                    for build in order:
                        collect(build, mix, mode, 0, 9, pair, delay)
            for case, n in SHAPES:
                for mode in ['cold', 'warm']:
                    for build in order:
                        collect(build, case, mode, n, 41, pair, 0)
            print('pair', pair, 'complete', flush=True)
    assert count == 4 * (6 * 2 * 9 * 40 + 10 * 2 * 41) == 20560
    identity.update(raw_count=count, postflight=run(ssh + ['uptime; free -m; df -h /; ps']))
    (OUT / 'next-header-identity.json').write_text(json.dumps(identity, indent=2) + '\n')


if __name__ == '__main__':
    main()
