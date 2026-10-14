"""J bounded serial matched A/B. No concurrent guest experiments.
Usage: hotspot_ab.py HOST KEY KNOWN LABEL BASE CAND [chunks|regression]
Binary paths are local; each upload is SHA256 read back. Preserve raw.
"""
import gzip
import hashlib
import json
from pathlib import Path
import shlex
import subprocess
import sys
ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'bench/results/j-hotspots'
def run(args):
    return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT)
def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
    host, key, known, label, base, cand, *extra = sys.argv[1:]
    kind = extra[0] if extra else 'chunks'
    opts = ['-i', key, '-o', f'UserKnownHostsFile={known}', '-o', 'StrictHostKeyChecking=yes', '-o', 'BatchMode=yes']
    ssh = ['ssh', *opts, host]
    remote = '/root/minipy-native-j'
    OUT.mkdir(parents=True, exist_ok=True)
    raw = OUT / f'{label}.jsonl.gz'
    assert not raw.exists(), 'preserve raw'
    ident = {'head': run(['git', 'rev-parse', 'HEAD']).strip(), 'patch': run(['git', 'diff', '--binary']), 'binaries': {}, 'commands': [], 'kind': kind, 'preflight': run(ssh + ['uname -a; uptime; free -m; df -h /; ps']), 'rust': run(['rustc', '+1.96.0', '--version']).strip(), 'zig': run(['zig', 'version']).strip()}
    ident['source_sha256'] = {str(p.relative_to(ROOT)): sha(p) for pattern in ['src/*.rs', 'bench/src/bin/interpreter_ab.rs', 'tools/hotspot_ab.py', 'Cargo*', 'bench/Cargo*'] for p in ROOT.glob(pattern) if p.is_file()}
    run(ssh + [f'mkdir -p {remote}'])
    for build, path in [('base', base), ('candidate', cand)]:
        p = Path(path)
        run(['scp', *opts, str(p), f'{host}:{remote}/{build}'])
        assert run(ssh + [f'sha256sum {remote}/{build}']).split()[0] == sha(p)
        ident['binaries'][build] = {'sha256': sha(p), 'bytes': p.stat().st_size}
    cells = [(c, m, 'mixed-cold', 0) for c in [64, 256, 1024] for m in ['light', 'scan']] if kind == 'chunks' else [(256, m, mode, delay) for m in ['light', 'scan'] for mode, delay in [('mixed-cold', 0), ('mixed-cold', 1000), ('mixed-warm', 0)]]
    count = 0
    with gzip.open(raw, 'wt') as f:
        def collect(build, engine, case, mode, n, reps, ri, chunk, delay):
            nonlocal count
            cmd = f'MINIPY_CHUNK={chunk} MINIPY_TOOL_DELAY_US={delay} ' + shlex.join([f'{remote}/{build}', engine, case, mode, str(n), str(reps)])
            ident['commands'].append(cmd)
            rs = [json.loads(l) for l in run(ssh + [cmd]).splitlines()]
            assert len(rs) == reps * (40 if mode.startswith('mixed-') else 1)
            assert all(r['completed'] for r in rs)
            for r in rs:
                r.update(build=build, round=ri, chunk=chunk)
                f.write(json.dumps(r, separators=(',', ':')) + '\n')
            f.flush()
            count += len(rs)
        for ri in range(4):
            order = ['base', 'candidate'] if ri in [0, 3] else ['candidate', 'base']
            for chunk, mix, mode, delay in cells:
                for build in order:
                    collect(build, 'interpreter', mix, mode, 0, 9, ri, chunk, delay)
            print(label, 'pair', ri, 'complete', flush=True)
        for case, n in [('scalar', 0), ('weighted', 100), ('weighted', 10000), ('stateful', 100000)]:
            for mode in ['cold', 'warm']:
                for build in ['candidate', 'base']:
                    collect(build, 'interpreter', case, mode, n, 41, 0, 256, 0)
        if kind == 'regression':
            for engine in ['native', 'reference', 'monty']:
                for mix in ['light', 'scan']:
                    collect('candidate', engine, mix, 'mixed-cold', 0, 9, 0, 256, 0)
    expected = 4 * len(cells) * 2 * 9 * 40 + 8 * 2 * 41 + (3 * 2 * 9 * 40 if kind == 'regression' else 0)
    assert count == expected
    ident.update(raw_count=count, postflight=run(ssh + ['uptime; free -m; df -h /; ps']))
    (OUT / f'{label}-identity.json').write_text(json.dumps(ident, indent=2) + '\n')
if __name__ == '__main__':
    main()
