"""Reproduce or verify the four-cell wake/yield interaction diagnostic."""
import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import statistics
import subprocess


def summarize(out):
    rows = []
    for pair in range(3):
        for arm in ['io', 'time']:
            rs = list(csv.DictReader(io.StringIO((out/f'{pair}-{arm}.csv').read_text())))
            assert len(rs) == 12
            rows.extend(dict(r, pair=pair, arm=arm) for r in rs)
    summary = []
    for key in [(0, 0), (0, 3), (2, 0), (2, 3)]:
        means = {}
        for arm in ['io', 'time']:
            means[arm] = []
            for pair in range(3):
                rs = [r for r in rows if (int(r['detached']),int(r['yields'])) == key and r['arm'] == arm and r['pair'] == pair]
                assert len(rs) == 3
                means[arm].append(sum(int(r['total_ns']) for r in rs)/sum(int(r['iterations']) for r in rs)/1000)
        summary.append(dict(detached=key[0], yields=key[1], process_means_us=means))
    return len(rows), summary


def main():
    p = argparse.ArgumentParser()
    p.add_argument('binary', type=Path)
    p.add_argument('out', type=Path)
    p.add_argument('--verify', action='store_true')
    a = p.parse_args()
    if not a.verify:
        a.out.mkdir(parents=True, exist_ok=True)
        for pair in range(3):
            for arm in (['io','time'] if pair % 2 == 0 else ['time','io']):
                text = subprocess.check_output([str(a.binary.resolve())]+(['--io'] if arm == 'io' else []), text=True, timeout=30)
                (a.out/f'{pair}-{arm}.csv').write_text(text)
        count, summary = summarize(a.out)
        result = dict(rows=count, summary=summary,
                      binary_sha256=hashlib.sha256(a.binary.read_bytes()).hexdigest(),
                      source_sha256=hashlib.sha256(Path('examples/al_wake_probe.rs').read_bytes()).hexdigest())
        (a.out/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
    count, summary = summarize(a.out)
    saved = json.loads((a.out/'summary.json').read_text())
    assert (count, summary) == (saved['rows'], saved['summary'])
    assert saved['source_sha256'] == hashlib.sha256(Path('examples/al_wake_probe.rs').read_bytes()).hexdigest()
    for r in summary:
        print(r['detached'], r['yields'], {a:statistics.median(v) for a,v in r['process_means_us'].items()})
    print(f'exact summary match: {count} batch rows, 4 cells')


if __name__ == '__main__':
    main()
