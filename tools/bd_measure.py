#!/usr/bin/env python3
"""Run paired AY/fix process comparisons. Build each revision first, then pass binary paths."""
import argparse
import hashlib
import json
import statistics
import subprocess
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('kind', choices=('legacy', 'record'))
parser.add_argument('baseline', type=Path)
parser.add_argument('candidate', type=Path)
parser.add_argument('output', type=Path)
args = parser.parse_args()
files = {'baseline': args.baseline, 'candidate': args.candidate}
raw = [{'kind': 'identity', 'probe': args.kind,
        'sha256': {k: hashlib.sha256(p.read_bytes()).hexdigest() for k, p in files.items()},
        'rounds': 3, 'processes_per_revision_cell': 6, 'samples_per_process': 7,
        'feature': 'tool-json', 'timer': 'source-to-result including preparation in cold; prepared outside warm'}]
for engine, shape, rows in ([('interpreter', 'weighted', 4096),
                             ('interpreter', 'scalar', 0), ('native', 'weighted', 4096)]
                            if args.kind == 'legacy' else [(None, None, 4096)]):
    for round_id in range(3):
        for order in (('baseline', 'candidate'), ('candidate', 'baseline')):
            for name in order:
                if args.kind == 'legacy':
                    modes = ('cold', 'warm')
                    runs = [[str(files[name]), engine, shape, mode, str(rows), '7'] for mode in modes]
                else:
                    runs = [[str(files[name]), '7', str(rows)]]
                for command in runs:
                    result = subprocess.run(command, check=True, capture_output=True, text=True)
                    for item in map(json.loads, result.stdout.splitlines()):
                        if item['kind'] != ('speed' if args.kind == 'legacy' else 'sample'):
                            continue
                        assert (args.kind == 'record' or
                                (item['engine'] == engine and item['case'] == shape and item['completed']))
                        raw.append({'kind': 'sample', 'revision': name, 'round': round_id,
                                    'engine': item['engine'], 'shape': shape if shape else item['engine'],
                                    'mode': item['mode'], 'rows': rows,
                                    'ns': item['wall_ns'] if args.kind == 'legacy' else item['ns'],
                                    'cpu_ns': item.get('cpu_ns'), 'prepare_ns': item.get('prepare_ns'),
                                    'execute_ns': item.get('execute_ns')})
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_text(''.join(json.dumps(x, separators=(',', ':')) + '\n' for x in raw))
groups = sorted({(r['engine'], r['shape'], r['mode']) for r in raw if r['kind'] == 'sample'})
for group in groups:
    vals = {name: [r['ns'] for r in raw if r['kind'] == 'sample' and
                   (r['engine'], r['shape'], r['mode']) == group and r['revision'] == name]
            for name in files}
    assert len(vals['baseline']) == len(vals['candidate']) == 42
    print(group, {name: round(statistics.median(v)) for name, v in vals.items()})
print(f'{args.output}: {len(raw)-1} samples')
