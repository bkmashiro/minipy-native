#!/usr/bin/env python3
"""Collect all actually linked tape text, including LLVM-outlined run bodies."""
import gzip
import json
import re
import subprocess
from co_count_collect import OUT, OBJDUMP
from perf_workbench import SCRATCH, locked, sha


def main():
    path = OUT / 'machine.json.gz'
    assert not path.exists()
    result = {}
    with locked():
        for arm in 'AB':
            binary = SCRATCH / f'co-{arm.lower()}'
            symbols = subprocess.check_output([OBJDUMP, '--syms', str(binary)], text=True)
            selected = sorted({s.split()[-1] for s in symbols.splitlines() if ' F ' in s and 'interpreter4tape' in s})
            rows = []
            for symbol in selected:
                cmd = [OBJDUMP, '--disassemble-symbols=' + symbol, str(binary)]
                assembly = subprocess.check_output(cmd, text=True)
                instructions = re.findall(r'^\s*[0-9a-f]+:\s+(.*)$', assembly, re.M)
                rows.append(dict(symbol=symbol, command=cmd, instructions=len(instructions), bytes=4 * len(instructions), assembly=assembly))
            result[arm] = dict(binary_sha256=sha(binary), binary_bytes=binary.stat().st_size, rows=rows)
    with gzip.open(path, 'xt') as f:
        json.dump(result, f, indent=2)
    for arm in 'AB':
        print(arm, 'binary', result[arm]['binary_bytes'])
        for r in result[arm]['rows']:
            print(r['symbol'], r['instructions'], r['bytes'])
            if 'scalar_count' in r['symbol']:
                print(r['assembly'])

if __name__ == '__main__':
    main()
