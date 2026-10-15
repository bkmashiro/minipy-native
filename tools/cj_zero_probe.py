#!/usr/bin/env python3
"""Separate CJ allocation/layout diagnosis; restores temporary source overlays."""
import gzip
import json
import os
from pathlib import Path
import re
import subprocess
from perf_workbench import ROOT, locked, sha

OUT = ROOT / 'evidence/cj-zero-buffer'
BASE = 'e1ee1af7cb7e016504d4dac56e1e05199799b7c2'


def main():
    output = OUT / 'diagnostic.json.gz'
    if output.exists():
        raise ValueError('refuse to overwrite diagnosis')
    assert os.path.ismount('/Volumes/M.2')
    assert (ROOT / 'target').is_symlink()
    assert (ROOT / 'target').resolve() == Path('/Volumes/M.2/caches/minipy-native-target')
    env = dict(os.environ, CARGO_BUILD_JOBS='1', RUSTUP_AUTO_INSTALL='0',
               CARGO_TARGET_DIR=str(ROOT / 'target'))
    rows = []
    def run(cmd):
        r = subprocess.run(cmd, cwd=ROOT, env=env, text=True, capture_output=True)
        rows.append(dict(command=cmd, exit_code=r.returncode, stdout=r.stdout, stderr=r.stderr))
        if r.returncode:
            print(r.stdout[-4000:], r.stderr[-4000:])
        r.check_returncode()
        return r.stdout
    def apply(path, reverse=False):
        subprocess.run(['git', 'apply', *(['--reverse'] if reverse else []), str(path)],
                       cwd=ROOT, check=True)
    with locked():
        sources = {str(p.relative_to(ROOT)): sha(p) for p in (ROOT / 'src').rglob('*.rs')}
        result: dict = dict(base=BASE, host=run(['uname', '-a']),
                      rust=run(['rustc', '+1.96.0', '--version']), arms={}, attempts=rows,
                      inputs={str(p.relative_to(ROOT)): sha(p) for p in [
                          OUT/'candidate.patch', OUT/'diagnostic.patch', OUT/'allocation.rs',
                          ROOT/'evidence/ci-layout/probe.rs', Path(__file__)]})
        candidate = OUT / 'candidate.patch'
        overlay = OUT / 'diagnostic.patch'
        # Baseline is exact source, not an environment-variable approximation.
        apply(candidate, True)
        try:
            for arm in ['A', 'B']:
                for p in sources:
                    if arm == 'A':
                        expected = subprocess.check_output(['git', 'show', f'{BASE}:{p}'], cwd=ROOT)
                        assert (ROOT/p).read_bytes() == expected, p
                    else:
                        assert sha(ROOT/p) == sources[p], p
                identity = {p: sha(ROOT/p) for p in sources}
                apply(overlay)
                start = len(rows)
                try:
                    build = run(['cargo', '+1.96.0', 'test', '--offline', '--locked', '--release',
                                 '--features', 'tool-json', '--lib', '--no-run', '--message-format=json'])
                    artifacts = [json.loads(s) for s in build.splitlines() if s.startswith('{')]
                    bins = [r['executable'] for r in artifacts if r.get('executable')
                            and r.get('target', {}).get('name') == 'minipy_native']
                    assert len(bins) == 1
                    binary = bins[0]
                    rows[-1]['stdout'] = json.dumps([r for r in artifacts if r.get('executable')])
                    for test in ['interpreter::ci_probe::ci_layout', 'codegen::tests::ci_native',
                                 'cj_alloc::cj_allocations']:
                        run([binary, '--ignored', '--exact', test, '--nocapture', '--test-threads=1'])
                    objdump = '/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin/llvm-objdump'
                    syms = run([objdump, '--syms', binary])
                    selected = sorted({s.split()[-1] for s in syms.splitlines() if ' F ' in s
                                       and '__ZN13minipy_native11interpreter14execute_script28_' in s})
                    assert selected, 'missing actual async dispatcher symbol'
                    asm = run([objdump, '--disassemble-symbols='+','.join(selected), binary])
                    instructions = len(re.findall(r'^\s*[0-9a-f]+:', asm, re.M))
                    assert instructions > 10
                    result['arms'][arm] = dict(sources=identity, binary_sha256=sha(binary),
                                              dispatcher_symbols=selected, dispatcher_instructions=instructions,
                                              records=rows[start:])
                finally:
                    apply(overlay, True)
                if arm == 'A':
                    apply(candidate)
        finally:
            # If A failed, restore B; otherwise B is already present.
            if sha(ROOT/'src/interpreter.rs') != sources['src/interpreter.rs']:
                apply(candidate)
            assert all(sha(ROOT/p) == h for p, h in sources.items())
            with gzip.open(output, 'xt') as f:
                json.dump(result, f, indent=2)
        print('diagnosis saved', output)
        for arm, data in result['arms'].items():
            print(arm, 'dispatcher instructions', data['dispatcher_instructions'])


if __name__ == '__main__':
    main()
