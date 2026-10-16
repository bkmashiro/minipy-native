#!/usr/bin/env python3
"""CO proportional full correctness verification, sharing the existing build lock."""
import gzip
import json
import os
import subprocess
from perf_workbench import ROOT, locked, sha


def main():
    output = ROOT / 'evidence/co-count/verification.json.gz'
    assert not output.exists()
    env = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / 'target'), CARGO_BUILD_JOBS='1', RUSTUP_AUTO_INSTALL='0')
    assert not any(env.get(k) for k in ('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS'))
    cargo = ['cargo', '+1.96.0', 'test', '--offline', '--locked', '--release']
    clippy = ['cargo', '+stable', 'clippy', '--offline', '--locked', '--release', '--all-targets']
    commands = [cargo, cargo + ['--features', 'tool-json'], cargo + ['--features', 'tool-json,debug-runtime'],
                clippy + ['--', '-D', 'warnings'],
                clippy + ['--features', 'tool-json', '--', '-D', 'warnings'],
                clippy + ['--features', 'tool-json,debug-runtime', '--', '-D', 'warnings'],
                ['cargo', '+stable', 'fmt', '--all', '--', '--check'],
                ['python3', '-m', 'unittest', 'discover', '-s', 'tools', '-p', 'test_perf_workbench.py', '-v'],
                ['git', 'diff', '--check']]
    rows = []
    with locked():
        sources = {str(p.relative_to(ROOT)): sha(p) for p in sorted((ROOT / 'src').rglob('*.rs')) + [ROOT / 'tests/co_count.rs', ROOT / 'tests/perf_workbench.rs', ROOT / 'Cargo.toml', ROOT / 'Cargo.lock']}
        for command in commands:
            r = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True)
            rows.append(dict(command=command, exit_code=r.returncode, stdout=r.stdout, stderr=r.stderr))
            print('exit', r.returncode, command, flush=True)
            if r.returncode:
                print(r.stdout, r.stderr)
                break
        with gzip.open(output, 'xt') as f:
            json.dump(dict(sources=sources, records=rows), f, indent=2)
    assert len(rows) == len(commands) and all(r['exit_code'] == 0 for r in rows)

if __name__ == '__main__':
    main()
