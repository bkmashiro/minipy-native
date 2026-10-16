#!/usr/bin/env python3
"""Read-only CN evidence verifier and descriptive reduction. No builds or timing."""
import gzip
import json
import re
import subprocess
from pathlib import Path

from perf_workbench import ROOT, reduce, validate

OUT = ROOT / 'evidence/cn-pgo'


def summarize():
    profile = json.loads((OUT / 'profile.json').read_text())
    identities = {name: json.loads((OUT / (name + '-identity.json')).read_text())
                  for name in ('baseline', 'instrumented', 'candidate')}
    assert not set(profile['training_cases']) & set(profile['holdout_cases'])
    assert identities['baseline']['rustflags'] == ''
    assert identities['instrumented']['rustflags'].startswith('-Cprofile-generate=')
    assert identities['candidate']['rustflags'].startswith('-Cprofile-use=')
    assert identities['candidate']['rustflags'].endswith(' -Cllvm-args=-pgo-warn-missing-function')
    assert len({i['binary_sha256'] for i in identities.values()}) == 3
    for i in identities.values():
        assert i['sources'] == profile['sources']
        assert i['commit'] == profile['base']
        assert i['target'] == profile['target']
    import hashlib
    for path, digest in profile['sources'].items():
        blob = subprocess.check_output(['git', 'show', profile['base'] + ':' + path], cwd=ROOT)
        assert hashlib.sha256(blob).hexdigest() == digest, path
    campaigns = {key: reduce(OUT / (key + '.jsonl.gz')) for key in ('ab', 'aa')}
    for key, result in campaigns.items():
        assert result['complete'] and result['diagnostic'] is False
        assert result['aa'] is (key == 'aa')
        assert result['observed_processes'] == 40 and result['measured_samples'] == 480
        with gzip.open(OUT / (key + '.jsonl.gz'), 'rt') as f:
            manifest = json.loads(next(f))
        assert manifest['rounds'] == 1 and manifest['reps'] == 12
        assert manifest['cells'] == [[b, c] for b in ('tape', 'native') for c in profile['holdout_cases']]
        for arm in 'AB':
            expected = identities['baseline' if arm == 'A' or key == 'aa' else 'candidate']
            actual = dict(manifest['arms'][arm])
            actual.pop('path')
            assert actual == expected
    with gzip.open(OUT / 'execution.json.gz', 'rt') as f:
        records = json.load(f)
    assert all(r['exit_code'] == 0 for r in records)
    training = [r for r in records if Path(r['command'][0]).name == 'instrumented']
    assert len(training) == 10
    for r in training:
        rows = [json.loads(s.partition('PW ')[2]) for s in r['stdout'].splitlines() if 'PW ' in s]
        validate(rows, 24, False)
    suites = [r for r in records if r['command'][:4] == ['cargo', '+1.96.0', 'test', '--release']]
    assert len(suites) == 2
    for r, expected in zip(suites, ((74, 0, 3), (132, 0, 11))):
        totals = [tuple(map(int, m)) for m in re.findall(
            r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored', r['stdout'])]
        assert tuple(map(sum, zip(*totals))) == expected
    inspection = json.loads((OUT / 'inspection.json').read_text())
    for name, machine in inspection['machines'].items():
        assert machine['binary_sha256'] == identities[name]['binary_sha256']
        assert bool(machine['profile_symbols']) == (name == 'instrumented')
    # Cargo JSON renders one warning twice (message and rendered); count structured
    # messages, not substring occurrences. Non-JSON test builds emit plain stderr.
    missing_profile = {}
    for r in records:
        if r['command'][:3] != ['cargo', '+1.96.0', 'test']:
            continue
        if '--message-format=json' in r['command']:
            messages = [json.loads(s) for s in r['stdout'].splitlines() if s.startswith('{')]
            count = sum('no profile data available for function' in m.get('message', {}).get('message', '')
                        for m in messages if m.get('reason') == 'compiler-message')
        else:
            count = r['stderr'].count('no profile data available for function')
        missing_profile[' '.join(r['command'])] = count
    by_aa = {(c['backend'], c['case']): c for c in campaigns['aa']['cells']}
    cells = []
    for cell in campaigns['ab']['cells']:
        aa = by_aa[(cell['backend'], cell['case'])]
        cells.append(dict(backend=cell['backend'], case=cell['case'],
                          baseline_ns=cell['medians_ns']['A'], candidate_ns=cell['medians_ns']['B'],
                          b_over_a=cell['b_over_a'], aa_b_over_a=aa['b_over_a'],
                          processes_ab=cell['processes'], processes_aa=aa['processes']))
    return dict(base=profile['base'], decision='defer production PGO; keep unchanged release baseline',
                processes_per_campaign=40, measured_requests_per_campaign=480,
                training_cases=profile['training_cases'], holdout_cases=profile['holdout_cases'],
                profile_bytes=profile['profile_bytes'], missing_profile_warnings_by_command=missing_profile,
                binaries={k: {p: v[p] for p in ('binary_bytes', 'binary_sha256', 'build_wall_ns')}
                          for k, v in identities.items()},
                startup_fixture_median_ns={k: v['startup_median_ns'] for k, v in campaigns.items()},
                cells=cells)


if __name__ == '__main__':
    print(json.dumps(summarize(), indent=2))
