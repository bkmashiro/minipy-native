#!/usr/bin/env python3
"""CO bounded source-reversible diagnostics and exact formal machine code."""
import gzip
import json
import os
from pathlib import Path
import re
import subprocess
from perf_workbench import ROOT, SCRATCH, locked, sha

OUT = ROOT / 'evidence/co-count'
OBJDUMP = '/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin/llvm-objdump'
ENV = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / 'target'), CARGO_BUILD_JOBS='1', RUSTUP_AUTO_INSTALL='0')


def main():
    output = OUT / 'diagnostic.json.gz'
    assert not output.exists()
    assert not any(ENV.get(k) for k in ('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS'))
    records = []
    def run(cmd):
        r = subprocess.run(cmd, cwd=ROOT, env=ENV, text=True, capture_output=True)
        records.append(dict(command=cmd, exit_code=r.returncode, stdout=r.stdout, stderr=r.stderr))
        if r.returncode:
            print(r.stdout, r.stderr)
        r.check_returncode()
        return r.stdout
    patch = OUT / 'candidate.patch'
    overlay = OUT / 'diagnostic.patch'
    raw = {}
    with locked():
        assert os.path.ismount('/Volumes/M.2')
        assert (ROOT / 'target').resolve() == Path('/Volumes/M.2/caches/minipy-native-target')
        original = {str(p.relative_to(ROOT)): sha(p) for p in (ROOT / 'src').rglob('*.rs')}
        for arm in 'AB':
            records = []
            if arm == 'A':
                run(['git', 'apply', '--reverse', str(patch)])
            try:
                sources = {str(p.relative_to(ROOT)): sha(p) for p in (ROOT / 'src').rglob('*.rs')}
                run(['git', 'apply', str(overlay)])
                try:
                    build = run(['cargo', '+1.96.0', 'test', '--offline', '--locked', '--release', '--features', 'tool-json,interpreter-profile', '--lib', '--no-run', '--message-format=json'])
                    artifacts = [json.loads(s) for s in build.splitlines() if s.startswith('{')]
                    binaries = [r['executable'] for r in artifacts if r.get('executable') and r.get('target', {}).get('name') == 'minipy_native']
                    assert len(binaries) == 1
                    records[-1]['stdout'] = json.dumps([r for r in artifacts if r.get('executable')])
                    run([binaries[0], '--ignored', '--exact', 'interpreter::co_probe::co_probe', '--nocapture', '--test-threads=1'])
                    diagnostic_hash = sha(binaries[0])
                finally:
                    run(['git', 'apply', '--reverse', str(overlay)])
                if arm == 'A':
                    run(['cargo', '+1.96.0', 'test', '--offline', '--locked', '--release', '--test', 'co_count'])
                binary = SCRATCH / f'co-{arm.lower()}'
                syms = run([OBJDUMP, '--syms', str(binary)])
                selected = sorted({s.split()[-1] for s in syms.splitlines() if ' F ' in s and ('interpreter4tape18run_until_boundary' in s or 'scalar_count' in s)})
                assert len(selected) == (1 if arm == 'A' else 3), selected
                records[-1]['stdout'] = '\n'.join(s for s in syms.splitlines() if any(k in s for k in selected))
                assembly = {}
                for symbol in selected:
                    text = run([OBJDUMP, '--disassemble-symbols=' + symbol, str(binary)])
                    instructions = re.findall(r'^\s*[0-9a-f]+:\s+(.*)$', text, re.M)
                    assembly[symbol] = dict(instructions=len(instructions), bytes=4 * len(instructions))
                raw[arm] = dict(sources=sources, diagnostic_binary_sha256=diagnostic_hash, formal_binary_sha256=sha(binary), binary_bytes=binary.stat().st_size, assembly=assembly, records=records)
            finally:
                if arm == 'A':
                    run(['git', 'apply', str(patch)])
        assert all(sha(ROOT / p) == h for p, h in original.items())
        raw['inputs'] = dict(candidate_sha256=sha(patch), overlay_sha256=sha(overlay), probe_sha256=sha(OUT / 'probe.rs'), environment={k: ENV.get(k) for k in ('RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_TARGET_DIR','CARGO_BUILD_JOBS','RUSTUP_AUTO_INSTALL')})
        with gzip.open(output, 'xt') as f:
            json.dump(raw, f, indent=2)
    for arm in 'AB':
        print(arm, raw[arm]['assembly'])
        for record in raw[arm]['records']:
            for line in record['stdout'].splitlines():
                if line.startswith('CO_'):
                    print(arm, line)

if __name__ == '__main__':
    main()
