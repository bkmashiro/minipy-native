#!/usr/bin/env python3
"""Bounded private schema/frontend diagnostic; not formal request timing."""
import gzip
import json
import os
from pathlib import Path
import subprocess
import sys

import perf_workbench as pw


def main():
    output = Path(sys.argv[1])
    if output.exists():
        raise ValueError('refuse to overwrite raw')
    with pw.locked():
        if not os.path.ismount('/Volumes/M.2') or (pw.ROOT / 'target').resolve() != Path('/Volumes/M.2/caches/minipy-native-target'):
            raise ValueError('expected external shared target')
        env = dict(os.environ, CARGO_TARGET_DIR=str(pw.ROOT / 'target'), CARGO_BUILD_JOBS='1')
        cmd = ['cargo', '+1.96.0', 'test', '--offline', '--locked', '--release', '--features',
               'tool-json', '--lib', '--no-run', '--message-format=json']
        build = subprocess.run(cmd, env=env, cwd=pw.ROOT, text=True, capture_output=True, check=True)
        artifacts = [json.loads(s) for s in build.stdout.splitlines() if s.startswith('{')]
        binaries = [r['executable'] for r in artifacts if r.get('reason') == 'compiler-artifact'
                    and r.get('executable')]
        if len(binaries) != 1:
            raise ValueError('expected one library test executable')
        binary = binaries[0]
        test = 'frontend::schema_probe::schema_prepare_diagnosis'
        run = [binary, '--ignored', '--exact', test, '--nocapture', '--test-threads=1']
        output.parent.mkdir(parents=True, exist_ok=True)
        with gzip.open(output, 'xt') as f:
            pw.emit(f, {'kind': 'manifest', 'diagnostic': True, 'build': cmd, 'run': run,
                        'commit': pw.command(['git', 'rev-parse', 'HEAD']),
                        'rust': pw.command(['rustc', '+1.96.0', '--version']),
                        'binary_sha256': pw.sha(binary),
                        'sources': {str(p.relative_to(pw.ROOT)): pw.sha(p) for p in
                                    sorted((pw.ROOT / 'src').rglob('*.rs')) + [Path(__file__).resolve()]}})
            for process in range(2):
                result = subprocess.run(run, env=env, cwd=pw.ROOT, text=True,
                                        capture_output=True, timeout=60)
                rows = [json.loads(s.partition('SCHEMA ')[2]) for s in result.stdout.splitlines() if 'SCHEMA ' in s]
                ok = result.returncode == 0 and len(rows) == 16 and {(r['round'], r['inputs']) for r in rows} == {(r, n) for r in range(8) for n in [0, 2]}
                pw.emit(f, {'kind': 'process', 'process': process, 'ok': ok,
                            'exit_code': result.returncode, 'rows': rows,
                            'stdout': result.stdout, 'stderr': result.stderr})
                if not ok:
                    raise ValueError('failed diagnostic, partial raw preserved')
            pw.emit(f, {'kind': 'end', 'processes': 2})
    print(output)


if __name__ == '__main__':
    main()
