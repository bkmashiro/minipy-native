#!/usr/bin/env python3
"""Replay CE raw request reductions and exact measured-source patch identities."""
import difflib
import gzip
import hashlib
import json
from pathlib import Path
import re
import subprocess

from perf_workbench import reduce

ROOT = Path(__file__).resolve().parents[1]
DIR = ROOT / 'evidence/ce-overflow'
BASE = 'e3055b2f85bd5872eb3aee1eb2ea0dd45e8e1ff2'


def blob(path):
    return subprocess.check_output(['git', 'show', f'{BASE}:{path}'], cwd=ROOT)


def apply(text, patch):
    old = text.splitlines(keepends=True)
    out, cursor = [], 0
    lines = patch.splitlines(keepends=True)
    i = 0
    while i < len(lines):
        m = re.match(r'@@ -(\d+)(?:,\d+)? \+\d+(?:,\d+)? @@', lines[i])
        if not m:
            i += 1
            continue
        start = int(m[1]) - 1
        assert start >= cursor
        out.extend(old[cursor:start])
        cursor = start
        i += 1
        while i < len(lines) and not lines[i].startswith('@@'):
            line = lines[i]
            if line.startswith((' ', '-')):
                assert old[cursor] == line[1:]
                cursor += 1
            if line.startswith((' ', '+')):
                out.append(line[1:])
            i += 1
    return ''.join(out + old[cursor:])


def main():
    original = blob('src/codegen.rs').decode()
    candidate = apply(original, (DIR / 'candidate.patch').read_text())
    # Strip only the two candidate production additions. Preserve the exact,
    # pre-rustfmt test diagnostic used in both measured source identities.
    a = candidate.index('        // Only infallible leaves')
    b = candidate.index('        match e {\n            Expr::Int(v)', a)
    baseline = candidate[:a] + candidate[b:]
    a = baseline.index('    fn arithmetic(&mut self')
    b = baseline.index('\n}\n#[cfg(test)]', a)
    baseline = baseline[:a] + baseline[b + 1:]
    baseline_patch = ''.join(difflib.unified_diff(
        original.splitlines(keepends=True), baseline.splitlines(keepends=True),
        fromfile='a/src/codegen.rs', tofile='b/src/codegen.rs'))
    assert apply(original, baseline_patch) == baseline
    results = {}
    for name in ('ab', 'aa'):
        path = DIR / f'{name}.jsonl.gz'
        result = reduce(path)
        assert result['complete'] and not result['diagnostic']
        assert result['observed_processes'] == 80 and result['measured_samples'] == 3840
        with gzip.open(path, 'rt') as f:
            manifest = json.loads(next(f))
        if name == 'aa':
            assert manifest['arms']['A'] == manifest['arms']['B']
        for arm, identity in manifest['arms'].items():
            for path, expected in identity['sources'].items():
                source = (baseline if arm == 'A' or name == 'aa' else candidate).encode() if path == 'src/codegen.rs' else blob(path)
                assert hashlib.sha256(source).hexdigest() == expected, (name, arm, path)
        results[name] = result
        for cell in result['cells']:
            print(name, cell['backend'], cell['case'], f"{cell['b_over_a']:.5f}")
    # Evidence generation only. No production source writes or timing reruns.
    (DIR / 'baseline.patch').write_text(baseline_patch)
    (DIR / 'summary.json').write_text(json.dumps(results, indent=2) + '\n')
    print('All measured source identities verified against base blobs + patches.')


if __name__ == '__main__':
    main()
