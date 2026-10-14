#!/usr/bin/env python3
"""Build on WSL with caller-held compute.lock. Drop non-executable custom sections."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

p = argparse.ArgumentParser()
p.add_argument('--toolchain', default='1.96.0')
p.add_argument('--output', default='.artifacts/playground')
a = p.parse_args()
root = Path(__file__).resolve().parents[1]
subprocess.run(['cargo', '+' + a.toolchain, 'rustc', '--locked', '--release', '--lib', '--crate-type', 'cdylib',
                '--target', 'wasm32-unknown-unknown', '--features', 'debug-runtime', '--',
                '-C', 'link-arg=--max-memory=268435456'], cwd=root, check=True)
target = Path(os.environ.get('CARGO_TARGET_DIR', root / 'target'))
raw = (target / 'wasm32-unknown-unknown/release/minipy_native.wasm').read_bytes()
assert raw[:8] == b'\0asm\1\0\0\0'
output = bytearray(raw[:8])
i = 8
while i < len(raw):
    start = i
    section = raw[i]
    i += 1
    size = shift = 0
    while True:
        b = raw[i]
        i += 1
        size |= (b & 127) << shift
        if b < 128:
            break
        shift += 7
    end = i + size
    assert end <= len(raw)
    if section != 0:
        output.extend(raw[start:end])
    i = end
out = root / a.output
out.mkdir(parents=True, exist_ok=True)
(out / 'minipy_native.wasm').write_bytes(output)
for name in ['engine.mjs', 'worker.mjs', 'samples.mjs', 'browser-test.mjs', 'smoke.html']:
    shutil.copyfile(root / 'web' / name, out / name)
files = sorted([root / 'Cargo.toml', root / 'Cargo.lock', *root.glob('src/**/*.rs'),
                *root.glob('web/*.mjs'), root / 'scripts/build_playground.py'])
manifest = {'toolchain': subprocess.check_output(['rustc', '+' + a.toolchain, '--version'], text=True).strip(),
            'features': ['debug-runtime'], 'target': 'wasm32-unknown-unknown',
            'wasm_bytes': len(output), 'unstripped_bytes': len(raw),
            'sha256': hashlib.sha256(output).hexdigest(),
            'sources': {str(f.relative_to(root)): hashlib.sha256(f.read_bytes()).hexdigest() for f in files}}
(out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(json.dumps({k:v for k,v in manifest.items() if k != 'sources'}, indent=2))
