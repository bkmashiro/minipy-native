"""One bounded AB/BA/BA/AB qualification; stdlib, no concurrent processes."""
import argparse
import csv
import hashlib
import io
import json
import os
from pathlib import Path
import statistics
import subprocess


def main():
    p = argparse.ArgumentParser()
    p.add_argument('binaries', type=Path)
    p.add_argument('out', type=Path)
    p.add_argument('--new-prefix', default='candidate')
    a = p.parse_args()
    a.out.mkdir(parents=True, exist_ok=True)
    raw = []
    identities = {}
    for pair, order in enumerate([['base', 'candidate'], ['candidate', 'base'],
                                  ['candidate', 'base'], ['base', 'candidate']]):
        for arm in order:
            for probe, driver in [('json', 'time'), ('fastpath', 'time'), ('fastpath', 'io')]:
                prefix = a.new_prefix if arm == 'candidate' else 'base'
                binary = (a.binaries / f'aj-{prefix}-{probe}').resolve()
                identities[binary.name] = hashlib.sha256(binary.read_bytes()).hexdigest()
                env = dict(os.environ)
                env.pop('MINIPY_IO_DRIVER', None)
                if driver == 'io':
                    env['MINIPY_IO_DRIVER'] = '1'
                r = subprocess.run([str(binary)], env=env, text=True, capture_output=True, check=True)
                (a.out / f'{pair}-{arm}-{probe}-{driver}.csv').write_text(r.stdout)
                rows = list(csv.DictReader(io.StringIO(r.stdout)))
                assert len(rows) == (99 if probe == 'json' else 12), len(rows)
                for row in rows:
                    raw.append(dict(row, pair=pair, snapshot=arm, probe=probe, driver=driver))
    keys = sorted({(r['probe'], r['driver'], r['workload'], r['arm'], r['lifecycle']) for r in raw})
    summary = []
    for key in keys:
        rs = [r for r in raw if (r['probe'], r['driver'], r['workload'], r['arm'], r['lifecycle']) == key]
        means = {}
        for snapshot in ['base', 'candidate']:
            means[snapshot] = []
            for pair in range(4):
                selected = [r for r in rs if r['snapshot'] == snapshot and r['pair'] == pair]
                n = sum(int(r['iterations']) for r in selected)
                assert n
                means[snapshot].append(sum(int(r['total_ns']) for r in selected) / n / 1000)
        # Same workload checksum per iteration, independent of repetitions/process.
        assert len({int(r['checksum']) / int(r['iterations']) for r in rs}) == 1
        old, new = map(statistics.median, [means['base'], means['candidate']])
        summary.append(dict(zip(['probe', 'driver', 'workload', 'engine', 'lifecycle'], key),
                            base_us=old, candidate_us=new, change_pct=(new / old - 1) * 100,
                            pair_change_pct=[(b/a-1)*100 for a,b in zip(means['base'], means['candidate'])],
                            process_means_us=means))
    # Per-process CSVs are the canonical raw, not duplicated as a second JSON dump.
    (a.out / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    (a.out / 'binary-hashes.json').write_text(json.dumps(identities, indent=2) + '\n')
    print(json.dumps(dict(processes=24, rows=len(raw), cells=len(summary))))
    for row in summary:
        if row['probe'] == 'fastpath' or row['workload'].startswith('json_'):
            print(row)


if __name__ == '__main__':
    main()
