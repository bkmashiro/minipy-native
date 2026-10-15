#!/usr/bin/env python3
"""CL separate layout and exact-formal-binary machine evidence; shared build lock."""
import gzip
import json
import os
from pathlib import Path
import re
import subprocess
from perf_workbench import ROOT, SCRATCH, locked, sha

OUT = ROOT / 'evidence/cl-dest'
OBJDUMP = '/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin/llvm-objdump'
ENV = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / 'target'), CARGO_BUILD_JOBS='1', RUSTUP_AUTO_INSTALL='0')

def main():
    output = OUT / 'diagnostic.json.gz'
    if output.exists():
        raise ValueError('refuse overwrite')
    patch = OUT / 'candidate.patch'
    overlay = ROOT / 'evidence/ci-layout/diagnostic.patch'
    records = []
    def run(cmd):
        r = subprocess.run(cmd, cwd=ROOT, env=ENV, text=True, capture_output=True)
        records.append(dict(command=cmd, exit_code=r.returncode, stdout=r.stdout, stderr=r.stderr))
        if r.returncode:
            print(r.stdout, r.stderr)
        r.check_returncode()
        return r.stdout
    raw = {}
    with locked():
        assert os.path.ismount('/Volumes/M.2') and (ROOT / 'target').resolve() == Path('/Volumes/M.2/caches/minipy-native-target')
        original = {str(p.relative_to(ROOT)): sha(p) for p in (ROOT / 'src').rglob('*.rs')}
        for arm in 'AB':
            records = []
            if arm == 'A':
                run(['git', 'apply', '--reverse', str(patch)])
            try:
                identity = {str(p.relative_to(ROOT)): sha(p) for p in (ROOT / 'src').rglob('*.rs')}
                run(['git', 'apply', str(overlay)])
                try:
                    build = run(['cargo', '+1.96.0', 'test', '--offline', '--locked', '--release', '--features', 'tool-json', '--lib', '--no-run', '--message-format=json'])
                    artifacts = [json.loads(s) for s in build.splitlines() if s.startswith('{')]
                    binaries = [r['executable'] for r in artifacts if r.get('executable') and r.get('target', {}).get('name') == 'minipy_native']
                    assert len(binaries) == 1
                    records[-1]['stdout'] = json.dumps([r for r in artifacts if r.get('executable')])
                    run([binaries[0], '--ignored', '--exact', 'interpreter::ci_probe::ci_layout', '--nocapture', '--test-threads=1'])
                    layout_hash = sha(binaries[0])
                finally:
                    run(['git', 'apply', '--reverse', str(overlay)])
                binary = SCRATCH / f'cl-{arm.lower()}'
                syms = run([OBJDUMP, '--syms', str(binary)])
                selected = sorted({s.split()[-1] for s in syms.splitlines() if ' F ' in s and 'interpreter4tape18run_until_boundary' in s})
                assert len(selected) == 1, selected
                # Retain only the selected symbol, not the entire symbol table.
                records[-1]['stdout'] = '\n'.join(s for s in syms.splitlines() if any(k in s for k in selected))
                text = run([OBJDUMP, '--disassemble-symbols=' + selected[0], str(binary)])
                n = len(re.findall(r'^\s*[0-9a-f]+:', text, re.M))
                assert n > 1000
                raw[arm] = dict(sources=identity, layout_binary_sha256=layout_hash, formal_binary_sha256=sha(binary), instructions=n, records=records)
            finally:
                if arm == 'A':
                    run(['git', 'apply', str(patch)])
        assert all(sha(ROOT / p) == h for p, h in original.items())
        raw['inputs'] = dict(candidate_sha256=sha(patch), overlay_sha256=sha(overlay), probe_sha256=sha(ROOT / 'evidence/ci-layout/probe.rs'))
        with gzip.open(output, 'xt') as f:
            json.dump(raw, f, indent=2)
    for arm in 'AB':
        print(arm, 'instructions', raw[arm]['instructions'])
        for r in raw[arm]['records']:
            for line in r['stdout'].splitlines():
                if line.startswith(('CI_LAYOUT', 'CI_CASE', 'CI_FUTURE')):
                    print(arm, line)

if __name__ == '__main__':
    main()
