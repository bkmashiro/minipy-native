#!/usr/bin/env python3
"""Check the small public CPython oracle; no private corpus or execution backend."""
import itertools
import json
from pathlib import Path
import platform

separators = ['\n', '\r', '\r\n', '\v', '\f', '\x1c', '\x1d', '\x1e', '\x85', '\u2028', '\u2029']
texts = ['', 'plain', 'café😀', '\u00a0', '\u001f'] + [
    a + sep + b for sep in separators
    for a, b in [('', ''), ('a', ''), ('', 'b'), ('a', 'b'), ('😀', 'é')]
] + ['a' + a + b + 'WARN:x' for a, b in itertools.product(separators, repeat=2)]
expected = {'python_splitlines': [{'text': t, 'lines': t.splitlines()} for t in texts]}
path = Path(__file__).resolve().parent / 'results/dr/splitlines-oracle.json'
assert json.loads(path.read_text()) == expected
print(f'{platform.python_implementation()} {platform.python_version()}: {len(texts)} splitlines cases verified')
