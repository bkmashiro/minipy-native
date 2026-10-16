#!/usr/bin/env python3
"""Read-only CR identities, CPU intervals, original chronology and separate reductions."""
import gzip
import json
from pathlib import Path
import subprocess

from cr_pgo_recheck import OUT, HOLDOUT, assess
from perf_workbench import ROOT, reduce, sha

CN = ROOT / 'evidence/cn-pgo'
CN_COMMIT = '44212e00e9161b479b6a367409087e82d3520190'


def verify():
    # Imported original evidence is byte-for-byte immutable, never relabelled.
    for path in sorted(CN.iterdir()):
        original = subprocess.check_output(['git', 'show', CN_COMMIT + ':' + str(path.relative_to(ROOT))], cwd=ROOT)
        assert original == path.read_bytes(), path
    collection = json.load(gzip.open(OUT / 'collection.json.gz', 'rt'))
    assert collection['complete'] and not collection['rejected']
    assert collection['monitor_thread_stopped']
    assert collection['collector_sha256'] == sha(ROOT / 'tools/cr_pgo_recheck.py')
    assert collection['driver_sha256'] == sha(ROOT / 'tools/perf_workbench.py')
    owned = set()  # Raw already excludes collector and top; top is ignored by assess.
    preblocks = ['Processes:' + b for b in collection['preflight']['stdout'].split('Processes:')[1:]]
    pre = [assess(b, 90, owned) for b in preblocks[1:]]
    assert len(pre) == 2 and all(not s['busy'] for s in pre)
    blocks = ['Processes:' + b for b in collection['monitor_text'].split('Processes:')[1:]]
    windows = []
    for sample in collection['monitor_samples']:
        i = sample['index']
        parsed = assess(blocks[i], 85, owned)
        assert parsed['idle_percent'] == sample['idle_percent']
        if not sample['valid_delta']:
            assert i == 0
            continue
        assert not parsed['busy'] and not sample['busy']
        # top snapshot CPU is a delta from the PREVIOUS snapshot, not the
        # current block's output-receipt envelope. Keep collector raw unchanged.
        windows.append(dict(index=i,
            approx_begin_epoch=collection['monitor_samples'][i - 1]['begin_epoch'],
            approx_end_epoch=sample['begin_epoch'], **parsed))
    assert len(windows) >= 2
    campaigns = {}
    old_campaigns = {}
    for key in ('ab', 'aa'):
        path = OUT / f'{key}.jsonl.gz'
        result = reduce(path)
        assert result['complete'] and not result['diagnostic']
        assert result['aa'] == (key == 'aa')
        assert result['observed_processes'] == 40 and result['measured_samples'] == 480
        manifest = json.loads(gzip.open(path, 'rt').readline())
        assert manifest['rounds'] == 1 and manifest['reps'] == 12
        assert manifest['cells'] == [[b, c] for b in ('tape', 'native') for c in HOLDOUT]
        assert manifest['entries'] == {'A': 'registered', 'B': 'registered'}
        assert manifest['driver_sha256'] == collection['driver_sha256']
        for arm in 'AB':
            name = 'baseline' if arm == 'A' or key == 'aa' else 'candidate'
            expected = json.loads((CN / f'{name}-identity.json').read_text())
            actual = dict(manifest['arms'][arm])
            actual.pop('path')
            assert actual == expected
            assert collection['arms'][name] == manifest['arms'][arm]
        rec = next(r for r in collection['records'] if r['name'] == key)
        assert rec['exit_code'] == 0
        overlaps = [w['index'] for w in windows if w['approx_begin_epoch'] <= rec['end_epoch']
                    and w['approx_end_epoch'] >= rec['start_epoch']]
        assert overlaps
        campaigns[key] = dict(start_epoch=rec['start_epoch'], end_epoch=rec['end_epoch'],
                              overlapping_cpu_windows=overlaps, **result)
        old_campaigns[key] = reduce(CN / f'{key}.jsonl.gz')
        assert old_campaigns[key]['complete']
    identities = {n: json.loads((CN / f'{n}-identity.json').read_text()) for n in ('baseline', 'candidate')}
    profile = json.loads((CN / 'profile.json').read_text())
    assert collection['profile_sha256'] == profile['profile_sha256']
    for meta in identities.values():
        assert meta['sources'] == profile['sources']
        for path, digest in meta['sources'].items():
            assert sha(ROOT / path) == digest
    old_execution = json.load(gzip.open(CN / 'execution.json.gz', 'rt'))
    chronology = {}
    for key in ('ab', 'aa'):
        path = CN / f'{key}.jsonl.gz'
        row = next(r for r in old_execution if any(str(path) == p for p in r['command']))
        chronology[key] = dict(gzip_creation_epoch_seconds=int.from_bytes(path.read_bytes()[4:8], 'little'),
                               command_wall_ns=row['wall_ns'], exact_request_wall_timestamps=False)
    comparisons = []
    for cell in campaigns['ab']['cells']:
        backend, case = cell['backend'], cell['case']
        lookup = lambda campaign: next(c for c in campaign['cells'] if c['backend'] == backend and c['case'] == case)
        comparisons.append(dict(backend=backend, case=case, cr_b_over_a=cell['b_over_a'],
            cr_aa_b_over_a=lookup(campaigns['aa'])['b_over_a'],
            cn_b_over_a=lookup(old_campaigns['ab'])['b_over_a'],
            cn_aa_b_over_a=lookup(old_campaigns['aa'])['b_over_a'],
            cr_a3_over_a0=cell['processes']['A'][1]['median_ns'] / cell['processes']['A'][0]['median_ns'],
            cr_b2_over_b1=cell['processes']['B'][1]['median_ns'] / cell['processes']['B'][0]['median_ns']))
    return dict(decision='retain PGO experiment and positive native holdout signal; defer production enablement',
        original_evidence_status='uncontrolled environment, potential interference unproven; do not pool with CR',
        cn_commit=CN_COMMIT, cn_chronology=chronology, identities=identities,
        profile_sha256=collection['profile_sha256'], preflight=pre,
        cpu_windows=windows, comparisons=comparisons, cr_campaigns=campaigns,
        cn_campaigns=old_campaigns,
        limitations=['1Hz coarse system CPU and top-12 competitors, no hard CPU exclusivity',
                    'top deltas cover preceding snapshot interval; receipt timestamps are approximate',
                    'only two processes per arm/cell, no precision or no-regression claim',
                    'current idle state cannot establish or exclude historical contamination',
                    'original and replacement campaigns reduced separately, no pooling'])


if __name__ == '__main__':
    print(json.dumps(verify(), indent=2))
