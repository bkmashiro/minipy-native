"""Bounded same-executable scheduler control. Raw CSVs are canonical."""
import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import statistics
import subprocess


def summarize(out, discovery=False):
    rows = []
    for pair in range(4):
        for arm in ['base', 'candidate', 'time']:
            path = out / f'{pair}-{arm}.csv'
            batch = list(csv.DictReader(io.StringIO(path.read_text())))
            expected = (27 if arm == 'time' else 39) if discovery else (33 if arm == 'time' else 45)
            assert len(batch) == expected, (path, len(batch))
            rows.extend(dict(r, pair=pair, arm=arm) for r in batch)
    assert len(rows) == (420 if discovery else 492)
    summary = []
    for key in sorted({(r['workload'], r['engine'], r['lifecycle']) for r in rows}):
        selected = [r for r in rows if (r['workload'], r['engine'], r['lifecycle']) == key]
        assert len({int(r['checksum']) / int(r['iterations']) for r in selected}) == 1
        for r in selected:
            assert int(r['responses']) == (2 * (int(r['iterations']) + 1) if key[0] == 'mixed' else 0)
        means = {}
        short = {}
        for arm in ['base', 'candidate', 'time']:
            means[arm], short[arm] = [], []
            for pair in range(4):
                rs = [r for r in selected if r['arm'] == arm and r['pair'] == pair]
                if not rs:
                    continue
                n = sum(int(r['iterations']) for r in rs)
                means[arm].append(sum(int(r['total_ns']) for r in rs) / n / 1000)
                short[arm].append(sum(int(r['short_total_ns']) for r in rs) / n / 1000)
        base, new = (statistics.median(means[a]) for a in ['base', 'candidate'])
        summary.append(dict(workload=key[0], engine=key[1], lifecycle=key[2],
                            process_means_us=means, short_process_means_us=short,
                            base_us=base, candidate_us=new, change_pct=(new/base-1)*100,
                            pair_change_pct=[(b/a-1)*100 for a,b in zip(means['base'],means['candidate'])]))
    return dict(rows=len(rows), cells=len(summary), summary=summary)


def main():
    p = argparse.ArgumentParser()
    p.add_argument('binary', type=Path)
    p.add_argument('out', type=Path)
    p.add_argument('--verify', action='store_true')
    p.add_argument('--discovery', action='store_true', help='verify the earlier 13-cell discovery run')
    a = p.parse_args()
    if a.verify:
        result = summarize(a.out, a.discovery)
        assert json.loads((a.out / 'summary.json').read_text()) == result
        print(f"exact summary match: {result['rows']} batch rows, {result['cells']} cells")
        return
    a.out.mkdir(parents=True, exist_ok=True)
    identity = dict(binary_sha256=hashlib.sha256(a.binary.read_bytes()).hexdigest(),
                    base_commit=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),
                    rustc=subprocess.check_output(['rustc','-Vv'],text=True),
                    system=subprocess.check_output(['uname','-a'],text=True),
                    probe_sha256=hashlib.sha256(Path('examples/al_io_probe/support.rs').read_bytes()).hexdigest(),
                    lock_sha256=hashlib.sha256(Path('Cargo.lock').read_bytes()).hexdigest(),
                    orders=[['base','candidate'],['candidate','base'],['candidate','base'],['base','candidate']],
                    candidate='Tokio current_thread event_interval=256; baseline=61; both enable_all')
    (a.out/'identity.json').write_text(json.dumps(identity,indent=2)+'\n')
    for pair, order in enumerate(identity['orders']):
        for arm in order + ['time']:
            flags = {'base':[], 'candidate':['--candidate'], 'time':['--time-only']}[arm]
            r = subprocess.run([str(a.binary.resolve())]+flags, text=True, capture_output=True, check=True, timeout=60)
            (a.out/f'{pair}-{arm}.csv').write_text(r.stdout)
    result = summarize(a.out)
    (a.out/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
    for r in result['summary']:
        print(r['workload'],r['engine'],r['lifecycle'],round(r['base_us'],3),round(r['candidate_us'],3),round(r['change_pct'],2),[round(v,2) for v in r['pair_change_pct']])


if __name__ == '__main__':
    main()
