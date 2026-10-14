"""The single F noise follow-up: same binaries, longer serial processes.
Usage identical to predicate_ab.py. No taskset in guest, so no affinity claimed.
"""
import gzip
import json
import shlex
import sys
from predicate_ab import OUT, run


def main():
    host, key, known, *alias = sys.argv[1:]
    opts = ['-i', key, '-o', f'UserKnownHostsFile={known}', '-o', 'StrictHostKeyChecking=yes', '-o', 'BatchMode=yes']
    if alias:
        opts += ['-o', f'HostKeyAlias={alias[0]}']
    ssh = ['ssh', *opts, host]
    raw = OUT / 'followup.jsonl.gz'
    assert not raw.exists()
    identity = {'reason': 'Unchanged scalar round cold deltas -3/+22/-24 percent; extend steady-process interval',
                'reps': 1000, 'stable_samples': '501..1000', 'affinity': 'none: taskset unavailable; no guest software installed',
                'preflight': run(ssh + ['uptime; free -m; ps']), 'commands': [], 'remote_sha256': {}}
    remote = '/root/minipy-native-f-predicate'
    original = json.loads((OUT / 'identity.json').read_text())
    for variant in ['baseline', 'candidate']:
        actual = run(ssh + [f'sha256sum {remote}/{variant}']).split()[0]
        assert actual == original['binaries'][variant]['sha256']
        identity['remote_sha256'][variant] = actual
    count = 0
    with gzip.open(raw, 'wt') as f:
        for round_id in range(3):
            cells = [('scalar', 0), ('weighted', 10000), ('nested', 10000)]
            if round_id % 2:
                cells.reverse()
            variants = ['baseline', 'candidate'] if round_id % 2 == 0 else ['candidate', 'baseline']
            for case, n in cells:
                for mode in ['cold', 'prepared']:
                    for variant in variants:
                        command = shlex.join([f'{remote}/{variant}', case, mode, str(n), '1000'])
                        identity['commands'].append(command)
                        rows = [json.loads(line) for line in run(ssh + [command]).splitlines()]
                        assert len(rows) == 1001 and [r['sample'] for r in rows] == list(range(1001))
                        for row in rows:
                            assert row['ok'] and row['case'] == case and row['rows'] == n and row['mode'] == mode
                            row.update(round=round_id, variant=variant)
                            f.write(json.dumps(row, separators=(',', ':')) + '\n')
                            count += 1
                        f.flush()
                        print(round_id, case, mode, variant, '1001 oracles passed', flush=True)
    assert count == 36036
    identity['rows'] = count
    identity['postflight'] = run(ssh + ['uptime; free -m; ps'])
    (OUT / 'followup-identity.json').write_text(json.dumps(identity, indent=2) + '\n')


if __name__ == '__main__':
    main()
