"""AD bounded paired campaign. WSL only, caller holds shared compute.lock.
python3 ad_campaign.py BASE smoke|formal
No previous cohort raw is pooled. Each cell has three independent processes.
"""
from collections import Counter
import gzip
import hashlib
import json
import os
from pathlib import Path
import resource
import subprocess
import sys
import time

COHORT = 'ad-corrected-json-prefork'
ARMS = ['cow', 'fork', 'fresh', 'resident']
SHAPES = [('scalar', 0)] + [(c, n) for c in ['weighted', 'stateful']
                                    for n in [100, 300, 1000, 3000, 10000, 30000, 100000]]
PHASES = ['process_startup_ns', 'runtime_init_ns', 'source_prepare_ns', 'execute_ns', 'reap_unassigned_ns']


def oracle(case, n):
    if case == 'scalar': return '734\nTrue\n'
    total = prev = 0
    for i in range(n):
        p, q = i * 37 % 1000, i % 9 - 2
        if q > 0 and p <= 700 and (case != 'stateful' or p >= prev):
            total += p * q
            if case == 'stateful': prev = p
    return f'{total + 17}\n'


def schedule(base):
    out = base / 'ad-corrected'
    work = []
    for ri in range(3):
        arms = ARMS[ri:] + ARMS[:ri]
        if ri % 2: arms.reverse()
        cells = [('speed', c, n, 11, 0) for c, n in SHAPES]
        cells += [('mixed', mix, 0, 5, d) for mix in ['light', 'scan'] for d in [0, 1000]]
        cells += [('hold', 'weighted', 10000, tasks, 0) for tasks in [1, 8, 32]]
        cells += [('first-request', c, n, 1, 0) for c, n in [('scalar', 0), ('weighted', 10000), ('stateful', 10000)]]
        for kind, c, n, reps, delay in cells:
            for arm in arms:
                mode = {'speed': 'cold', 'mixed': 'mixed-cold'}.get(kind, kind)
                expected = reps * 40 if kind == 'mixed' else reps if kind == 'speed' else (5 if arm == 'fork' else 4) if kind == 'hold' else 1
                work.append(dict(key=f'{kind}-{ri}-{arm}-{c}-{n}-{reps}-{delay}',
                    argv=['taskset', '-c', '2', str(out/'worker'), arm, c, mode, str(n), str(reps), str(delay), str(base/'source/fixtures'), str(base/'w-guest/runtime/dist/pysolate.wasm')],
                    kind=kind, expected=expected, discard=1 if kind in ['speed', 'mixed'] else 0,
                    round=ri, arm=arm, case=c, rows=n, reps=reps, delay_us=delay))
    assert len(work) == len({i['key'] for i in work}) == 300
    assert sum(i['expected'] for i in work) == 11769
    return work


def validate(rows, item):
    rs = [r for r in rows if r['kind'] == item['kind']]
    assert len(rs) == item['expected'], (item['key'], len(rs))
    assert sum(r['kind'] == 'initialization' for r in rows) == 1
    assert all(r['cohort'] == COHORT and r['command_key'] == item['key'] and r['round'] == item['round'] for r in rows)
    if item['kind'] in ['speed', 'mixed']:
        copies = 40 if item['kind'] == 'mixed' else 1
        assert Counter(r['sample'] for r in rs) == Counter({i: copies for i in range(item['reps'])})
        assert all(r['completed'] for r in rs)
        if item['kind'] == 'mixed':
            weights = [24, 8, 4, 2, 2] if item['case'] == 'light' else [8, 16, 8, 4, 4]
            shapes = [('scalar', 0), ('weighted', 100), ('stateful', 100), ('weighted', 10000), ('stateful', 100000)]
            for i in range(item['reps']):
                batch = [r for r in rs if r['sample'] == i]
                assert Counter((r['case'], r['rows']) for r in batch) == Counter(dict(zip(shapes, weights)))
                assert {r['arrival'] for r in batch} == set(range(40))
                assert len({(r['batch_wall_ns'], r['batch_cpu_ns']) for r in batch}) == 1
    elif item['kind'] == 'hold':
        phases = ['startup'] + (['empty_tree'] if item['arm'] == 'fork' else []) + ['load_wait', 'record_wait', 'reclaimed']
        assert [r['phase'] for r in rs] == phases
        pidsets = {r['phase']: set(r['physical']['tree_pids']) for r in rs}
        baseline = 'empty_tree' if item['arm'] == 'fork' else 'startup'
        assert pidsets[baseline] == pidsets['load_wait'] == pidsets['record_wait']
        assert pidsets['startup'] == pidsets['reclaimed']
        assert len(pidsets[baseline]) == (item['reps'] + 2 if item['arm'] == 'fork' else 2 if item['arm'] == 'resident' else 1)
        for r in rs:
            assert sum(p['pss_bytes'] for p in r['physical']['per_pid']) == r['physical']['pss_bytes']
            assert sum(p['rss_bytes'] for p in r['physical']['per_pid']) == r['physical']['rss_bytes']
    else:
        for r in rs:
            assert r['completed'] and r['stdout'] == oracle(item['case'], item['rows'])
            want = 0 if item['case'] == 'scalar' else 1
            assert (r['loads'], r['records']) == (want, want)
            assert sum(r[p] for p in PHASES) == r['wall_ns']
            assert all(r[p] >= 0 for p in PHASES)


def cpu():
    return sum(round((r.ru_utime + r.ru_stime) * 1e9) for r in
               [resource.getrusage(resource.RUSAGE_SELF), resource.getrusage(resource.RUSAGE_CHILDREN)])


def run(item, base):
    start, cost = time.perf_counter_ns(), cpu()
    p = subprocess.run(item['argv'], capture_output=True, text=True, timeout=600,
                       env={**os.environ, 'AD_NATIVE_SCRIPT': str(base/'ad-corrected/native.py')})
    end, cost = time.perf_counter_ns(), cpu() - cost
    if p.returncode: raise RuntimeError(f'exit {p.returncode}: {p.stderr}')
    rows = [json.loads(line) for line in p.stdout.splitlines()]
    for r in rows:
        if r['kind'] == 'first-request':
            a, b, c, d = [r[k] for k in ['init_begin_ns', 'ready_ns', 'prepared_ns', 'executed_ns']]
            assert start <= a <= b <= c <= d <= end
            r.update(wall_ns=end-start, cpu_ns=cost, process_startup_ns=a-start,
                     runtime_init_ns=b-a, source_prepare_ns=c-b, execute_ns=d-c,
                     reap_unassigned_ns=end-d, launch_ns=start, reaped_ns=end)
        r.update(round=item['round'], command_key=item['key'], cohort=COHORT, discard=item['discard'])
    validate(rows, item)
    return rows


def main(base, mode):
    out = base / 'ad-corrected'
    work = schedule(base)
    files = [out/'worker', out/'native.py', out/'harness/main.go', out/'harness/native.go', out/'harness/go.mod', out/'harness/go.sum', Path(__file__), base/'w-guest/runtime/dist/pysolate.wasm', base/'w-guest/runtime/runner.go', base/'w-guest/runtime/guest/runtime.c']
    files += list((base/'source/fixtures').glob('common_*.py'))
    hashes = {str(p): dict(sha256=hashlib.sha256(p.read_bytes()).hexdigest(), bytes=p.stat().st_size) for p in files}
    if mode == 'smoke':
        smoke = []
        for arm in ARMS:
            for kind, c, n in [('speed', 'weighted', 100000), ('mixed', 'scan', 0), ('hold', 'weighted', 10000), ('first-request', 'weighted', 10000)]:
                item = next(i.copy() for i in work if i['round'] == 0 and i['arm'] == arm and i['kind'] == kind and i['case'] == c and i['rows'] == n and (kind != 'hold' or i['reps'] == 8))
                if kind in ['speed', 'mixed']:
                    item['reps'] = 2 if kind == 'speed' else 1
                    item['argv'][8] = str(item['reps'])
                    item['expected'] = item['reps'] * (40 if kind == 'mixed' else 1)
                smoke.append(dict(command=item, observations=run(item, base)))
                print('smoke', arm, kind, 'pass', flush=True)
        (out/'smoke.json').write_text(json.dumps(smoke, indent=2)+'\n')
        return
    assert mode == 'formal' and (out/'smoke.json').exists()
    identity = out/'identity.json'
    if identity.exists():
        assert json.loads(identity.read_text())['files'] == hashes
    else:
        assert not list(out.glob('*.jsonl.gz'))
        identity.write_text(json.dumps(dict(cohort=COHORT, files=hashes, source_commit=os.environ['AD_SOURCE_COMMIT'],
            environment=subprocess.check_output(['bash','-lc','uname -a; python3 --version; go version; uptime; free -h; lscpu; ps -eo pid,comm,pcpu --sort=-pcpu'], text=True),
            locks=[str(base/'compute.lock'), str(base/'wsl-heavy.lock')], affinity=2), indent=2)+'\n')
    commands = out/'commands.json'
    if commands.exists(): assert json.loads(commands.read_text()) == work
    else: commands.write_text(json.dumps(work, indent=2)+'\n')
    status = []
    for ix, item in enumerate(work):
        raw = out/(item['key']+'.jsonl.gz')
        try:
            if raw.exists():
                with gzip.open(raw, 'rt') as f: rows = [json.loads(line) for line in f]
                validate(rows, item)
            else:
                rows = run(item, base)
                with gzip.open(raw, 'wt') as f:
                    for r in rows: f.write(json.dumps(r)+'\n')
            status.append(dict(key=item['key'], state='completed', rows=item['expected']))
        except Exception as exc:
            status.append(dict(key=item['key'], state='failed', error=repr(exc), scheduled_rows=item['expected']))
        (out/'status.json').write_text(json.dumps(status, indent=2)+'\n')
        print(ix+1, len(work), item['key'], status[-1]['state'], flush=True)
        if status[-1]['state'] != 'completed': raise RuntimeError(status[-1])
    assert all(hashlib.sha256(Path(p).read_bytes()).hexdigest() == h['sha256'] for p, h in hashes.items())
    print('DONE', len(status), sum(s['rows'] for s in status), flush=True)


if __name__ == '__main__':
    main(Path(sys.argv[1]).resolve(), sys.argv[2])
