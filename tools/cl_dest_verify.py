#!/usr/bin/env python3
"""CL bounded correctness, diagnostic frontier and final release verification."""
import gzip
import json
import os
from pathlib import Path
import subprocess
from perf_workbench import ROOT, locked, sha

OUT = ROOT / 'evidence/cl-dest'
ENV = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / 'target'), CARGO_BUILD_JOBS='1', RUSTUP_AUTO_INSTALL='0')

def main():
    output = OUT / 'verification.json.gz'
    if output.exists():
        raise ValueError('refuse overwrite')
    records = []
    def run(cmd):
        r = subprocess.run(cmd, cwd=ROOT, env=ENV, text=True, capture_output=True)
        records.append(dict(command=cmd, exit_code=r.returncode, stdout=r.stdout, stderr=r.stderr))
        print('exit', r.returncode, ' '.join(cmd), flush=True)
        if r.returncode:
            print(r.stdout, r.stderr)
        return r.returncode
    with locked():
        assert os.path.ismount('/Volumes/M.2') and (ROOT / 'target').resolve() == Path('/Volumes/M.2/caches/minipy-native-target')
        source = {str(p.relative_to(ROOT)): sha(p) for p in (ROOT / 'src').rglob('*.rs')}
        assert run(['rustfmt', '+stable', '--edition', '2024', 'tests/cl_dest.rs', 'evidence/cl-dest/bounds.rs']) == 0
        patch = OUT / 'bounds.patch'
        assert run(['git', 'apply', str(patch)]) == 0
        try:
            run(['cargo', '+1.96.0', 'test', '--offline', '--locked', '--release', '--features', 'tool-json,debug-runtime', '--lib', 'interpreter::tape::cl_bounds', '--', '--nocapture', '--test-threads=1'])
        finally:
            assert run(['git', 'apply', '--reverse', str(patch)]) == 0
            assert all(sha(ROOT / p) == h for p, h in source.items())
        for features in (None, 'tool-json,debug-runtime'):
            run(['cargo', '+1.96.0', 'test', '--offline', '--locked', '--release'] + (['--features', features] if features else []))
        for features in (None, 'tool-json,debug-runtime'):
            run(['cargo', '+1.96.0', 'clippy', '--offline', '--locked', '--release', '--all-targets'] + (['--features', features] if features else []) + ['--', '-D', 'warnings'])
        run(['cargo', '+stable', 'fmt', '--all', '--', '--check'])
        run(['python3', '-m', 'unittest', 'discover', '-s', 'tools', '-p', 'test_perf_workbench.py', '-v'])
        run(['git', 'diff', '--check'])
        raw = dict(sources=source, test_sha256=sha(ROOT / 'tests/cl_dest.rs'), bounds_sha256=sha(OUT / 'bounds.rs'), bounds_patch_sha256=sha(patch), records=records)
        with gzip.open(output, 'xt') as f:
            json.dump(raw, f, indent=2)
    assert all(r['exit_code'] == 0 for r in records), 'verification failure preserved in raw'

if __name__ == '__main__':
    main()
