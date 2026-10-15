#!/usr/bin/env python3
"""Replay the bounded CF combined acceptance, exact source identities and prior CC census."""
import gzip
import hashlib
import json
from pathlib import Path
import statistics
import subprocess

import cc_waiting_reduce as cc
import perf_workbench as pw

ROOT = Path(__file__).resolve().parents[1]
EVIDENCE = ROOT / 'evidence/cf-stage'
BASE = 'c8d28f07ed9fbeb26444c308642767f34d7d56bc'
FINAL = '1507ae8173842d980531f10abae1a1e92f398d49'
CASES = ['branch', 'escaped_join', 'detach_wait', 'madd_loop']
CHANGED = {'src/codegen.rs', 'src/interpreter.rs', 'src/interpreter/tape.rs',
           'src/json_view.rs', 'src/runtime.rs'}


def blob(commit, path):
    return subprocess.check_output(['git', 'show', f'{commit}:{path}'], cwd=ROOT)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def load(path):
    with gzip.open(path, 'rt') as f:
        return [json.loads(line) for line in f]


def main():
    subprocess.run(['git', 'merge-base', '--is-ancestor', BASE, FINAL], cwd=ROOT, check=True)
    summaries = {}
    manifests = {}
    for label in ('ab', 'aa'):
        path = EVIDENCE / f'{label}.jsonl.gz'
        manifest = load(path)[0]
        manifests[label] = manifest
        assert manifest['cells'] == [[b, c] for b in pw.BACKENDS for c in CASES]
        assert manifest['reps'] == 24 and manifest['rounds'] == 2
        assert manifest['aa'] == (label == 'aa')
        assert manifest['driver_sha256'] == pw.sha(ROOT / 'tools/perf_workbench.py')
        a, b = manifest['arms']['A'], manifest['arms']['B']
        assert a['sources'].keys() == b['sources'].keys()
        changed = {p for p in a['sources'] if a['sources'][p] != b['sources'][p]}
        assert changed == (CHANGED if label == 'ab' else set())
        for arm, identity in manifest['arms'].items():
            # Collection HEAD was FINAL for both builds. Source hashes, not that
            # label, identify the temporary pre-BW production overlay.
            assert identity['commit'] == FINAL
            assert not identity['diagnostic']
            snapshot = FINAL if label == 'ab' and arm == 'B' else BASE
            for p, value in identity['sources'].items():
                if p == 'tests/perf_workbench.rs':
                    expected = pw.sha(ROOT / p)  # identical CF adapter overlay
                else:
                    # Inert test-only module added by CD, absent from BASE and
                    # not compiled by either formal integration-test library.
                    version = FINAL if p == 'src/interpreter/liveness_tests.rs' else snapshot
                    expected = digest(blob(version, p))
                assert value == expected, (label, arm, p)
        summary = pw.reduce(path)
        assert summary['complete'] and not summary['diagnostic']
        assert summary['observed_processes'] == 64 and summary['measured_samples'] == 1536
        summaries[label] = summary
    assert manifests['ab']['arms']['A'] == manifests['aa']['arms']['A'] == manifests['aa']['arms']['B']
    # Replay old diagnostics only: these instrumented Ready requests are not
    # current CF phase measurements and are never mixed with formal timings.
    historical_path = ROOT / 'evidence/perf-workbench/diagnostic.jsonl.gz'
    historical = load(historical_path)
    requests = [r for p in historical if p.get('kind') == 'process' and p['ok']
                for r in p['rows'][2:]]
    assert len(requests) == 48
    phases = {field: statistics.median(r[field] for r in requests)
              for field in ('ns', 'prepare_ns', 'execute_including_code_release_ns')}
    phases['backend_ns'] = statistics.median(r['compiler']['backend_ns'] for r in requests)
    phases['median_request_backend_over_prepare'] = statistics.median(
        r['compiler']['backend_ns'] / r['prepare_ns'] for r in requests)
    result = {'base': BASE, 'final': FINAL, 'campaigns': summaries,
              'reused_cc_census': cc.reduce(), 'historical_bt_diagnostic': phases,
              'historical_bt_raw_sha256': pw.sha(historical_path)}
    (EVIDENCE / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    aa = {(c['backend'], c['case']): c['b_over_a'] for c in summaries['aa']['cells']}
    for cell in summaries['ab']['cells']:
        print(cell['backend'], cell['case'], 'A/B', round(cell['b_over_a'], 5),
              'A/A', round(aa[cell['backend'], cell['case']], 5), 'ns', cell['medians_ns'])
    print('Reused CC:', result['reused_cc_census']['tasks_per_arm'], 'tasks per arm')
    print('Historical BT instrumented phases:', phases)


if __name__ == '__main__':
    main()
