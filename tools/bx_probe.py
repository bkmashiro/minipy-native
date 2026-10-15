#!/usr/bin/env python3
"""Separate BX preparation/layout diagnostic. No formal request timing."""
import argparse
import json
import os
from pathlib import Path
import subprocess
from perf_workbench import ROOT, locked, sha


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('output', type=Path)
    args = p.parse_args()
    if args.output.exists():
        raise ValueError('refuse to overwrite evidence')
    cmd = ['cargo', '+1.96.0', 'test', '--offline', '--locked', '--release',
           '--features', 'tool-json', '--lib', 'bx_diagnostic', '--', '--ignored', '--nocapture']
    with locked():
        env = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / 'target'), CARGO_BUILD_JOBS='1')
        r = subprocess.run(cmd, cwd=ROOT, env=env, capture_output=True, text=True, check=True)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps({'command': cmd, 'stdout': r.stdout, 'stderr': r.stderr,
                                           'sources': {str(s.relative_to(ROOT)): sha(s) for s in
                                                       sorted((ROOT / 'src').rglob('*.rs'))}}, indent=2) + '\n')
    print(r.stdout)


if __name__ == '__main__':
    main()
