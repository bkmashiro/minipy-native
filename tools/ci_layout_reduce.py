#!/usr/bin/env python3
"""Replay CI layout/count evidence. No timing deltas or inferred PMU events."""
import gzip
import hashlib
import json
from pathlib import Path
import re
import subprocess
from ce_overflow_reduce import apply

ROOT = Path(__file__).resolve().parents[1]
DIR = ROOT / 'evidence/ci-layout'
BASE = 'ebee3c1dfd87b4835f8bc16a00c2734f3dd53d69'
NAMES = {'scalar','bound_tree','madd_short','quote','madd_loop','ready','branch','joined_sink','async_string'}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    with gzip.open(DIR / 'raw.json.gz', 'rt') as f:
        raw = json.load(f)
    identity, records = raw['identity'], raw['records']
    assert identity['commit'] == BASE
    assert len(records) == 6 and all(r['exit_code'] == 0 for r in records)
    patches = {}
    for part in (DIR / 'diagnostic.patch').read_text().split('diff --git ')[1:]:
        path = part.splitlines()[0].split()[1][2:]
        patches[path] = part
    assert set(patches) == {'src/codegen.rs','src/interpreter.rs'}
    for path, expected in identity['sources'].items():
        base = subprocess.check_output(['git','show',f'{BASE}:{path}'],cwd=ROOT)
        reconstructed = apply(base.decode(),patches[path]).encode() if path in patches else base
        assert digest(reconstructed) == expected, path
        assert (ROOT/path).read_bytes() == base, ('production changed', path)
    assert digest((DIR/'probe.rs').read_bytes()) == identity['probe_sha256']
    layout_text, native_text = records[1]['stdout'], records[2]['stdout']
    layout = json.loads(layout_text.split('CI_LAYOUT ',1)[1].splitlines()[0])
    cases = [json.loads(l.split('CI_CASE ',1)[1]) for l in layout_text.splitlines() if 'CI_CASE ' in l]
    assert len(cases) == 9 and {c['name'] for c in cases} == NAMES
    executed = re.findall(r'CI_EXECUTED (\w+) both status=Ok stdout=exact',layout_text)
    assert len(executed) == 9 and set(executed) == NAMES
    assert layout['op'] == 56 and layout['small_dest_op'] == 48
    native = {}
    for part in native_text.split('CI_CODE ')[1:]:
        name=part.splitlines()[0]
        assert name not in native
        byte_match = re.search(r'CI_BYTES (\d+)',part)
        assert byte_match is not None
        native[name] = {'code_bytes':int(byte_match[1]),
                        'clif_brif_sites':part.split('CI_CLIF',1)[1].count('brif')}
    assert set(native) == NAMES
    for c in cases:
        name = c['name']
        c.update(native[name])
        c['tape_used_bytes'] = layout['op'] * c['ops']
        c['tape_capacity_bytes'] = layout['op'] * c['capacity']
        c['future_bytes'] = {}
        for backend in ('tape','native'):
            future_match = re.search(rf'CI_FUTURE {name} {backend} (\d+)',layout_text)
            assert future_match is not None
            c['future_bytes'][backend] = int(future_match[1])
        c['source_derived_no_buffer_live_bytes_removable'] = 8*c['live_capacity'] if c['buffers']==0 else 0
        c['modeled_small_dest_tape_capacity_saving'] = (layout['op']-layout['small_dest_op'])*c['capacity']
    counts = [json.loads(l.split('CI_COUNTS ',1)[1]) for l in records[-1]['stdout'].splitlines() if 'CI_COUNTS ' in l]
    assert len(counts) == 6
    assert {(c['case'],c['rows']) for c in counts} == {(case,n) for case in ('quote','madd_loop') for n in (256,4096,65536)}
    for c in counts:
        assert c['dispatches'] == (4 if c['case']=='quote' else 6)*c['rows']+6
        assert c['fuel_reservations'] == c['rows']//256 and c['yields'] == c['rows']//256-1
    with gzip.open(DIR/'tape-aarch64.txt.gz','rt') as f:
        asm = f.read()
    instructions = re.findall(r'^([0-9a-f]+):',asm,re.M)
    assert len(instructions)>100
    for marker in ('#0x1000','#0x38','madd\tx19','str\tx8, [x20, #0x118]','br\tx9','smulh'):
        assert marker in asm, marker
    result = {'base':BASE,'kind':'separate layout, code and dynamic-count diagnosis, no timing',
              'layout':layout,'cases':cases,'loop_counts':counts,
              'tape_function_instruction_count':len(instructions),
              'tape_function_instruction_bytes':4*len(instructions),
              'hardware_misses_measured':False,
              'production_equal_base':True,
              'chosen_candidate':'omit buffer-only liveness allocation and traversal when admitted buffer count is zero',
              'secondary_deferred_candidate':'narrow Dest only, no operand pool, modeled Op 56 -> 48'}
    (DIR/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result,indent=2))


if __name__ == '__main__':
    main()
