#!/usr/bin/env python3
"""BY separate allocation/future diagnostic or serial verification under shared lock."""
import argparse
import json
import os
from pathlib import Path
import subprocess
from perf_workbench import ROOT, locked, sha


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('output', type=Path)
    p.add_argument('--verify', action='store_true')
    args = p.parse_args()
    if args.output.exists():
        raise ValueError('refuse to overwrite evidence')
    commands = ([['cargo', '+1.96.0', 'test', '--offline', '--locked', '--release'],
                 ['cargo', '+1.96.0', 'test', '--offline', '--locked', '--release', '--features', 'tool-json,debug-runtime']]
                if args.verify else
                [['cargo', '+1.96.0', 'test', '--offline', '--locked', '--release', '--features', 'tool-json',
                  '--test', 'by_dispatch', 'by_diagnostic', '--', '--ignored', '--nocapture', '--test-threads=1']])
    results = []
    with locked():
        env = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / 'target'), CARGO_BUILD_JOBS='1')
        for cmd in commands:
            r = subprocess.run(cmd, cwd=ROOT, env=env, capture_output=True, text=True)
            results.append({'command': cmd, 'exit_code': r.returncode, 'stdout': r.stdout, 'stderr': r.stderr})
            print(' '.join(cmd), 'exit', r.returncode, flush=True)
            if r.returncode:
                print(r.stdout[-6000:], r.stderr[-6000:])
                break
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps({'commands': results,
            'sources': {str(s.relative_to(ROOT)): sha(s) for s in sorted((ROOT / 'src').rglob('*.rs'))},
            'test_sha256': sha(ROOT / 'tests/by_dispatch.rs') if (ROOT / 'tests/by_dispatch.rs').exists() else None}, indent=2) + '\n')
    if any(r['exit_code'] for r in results):
        raise SystemExit(1)


if __name__ == '__main__':
    main()
