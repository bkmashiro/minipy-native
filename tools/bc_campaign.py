"""BC five-arm fresh-source campaign, WSL; run under shared compute.lock.
Usage: python3 tools/bc_campaign.py OUT [--smoke]
One prestarted JSON-line worker per arm/round except fresh CPython --once.
Request timer starts before encoding/sending, stops after reading/decoding response
(and process reap for spawn); source generation and oracle stay outside.
"""
import json
from pathlib import Path
import statistics
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / 'bench/target/release/bc_request'
PYTHON = Path('/usr/bin/python3')
PY_IMPL = ROOT / 'tools/bc_python.py'
ARMS = ('interpreter', 'native', 'monty', 'spawn', 'prefork')
SHAPES = [('scalar', 0)] + [('weighted', n) for n in (100, 300, 1000, 3000, 10000, 30000, 100000)]
WEIGHTED = (ROOT / 'fixtures/common_weighted.py').read_text()
SCALAR = (ROOT / 'fixtures/common_scalar.py').read_text()
assert WEIGHTED.count('quote.unit_price_cents <= price_limit') == 1
assert 'batch_id * 2 + price_limit' in SCALAR


def source(case, variant):
    if case == 'scalar':
        return SCALAR.replace('batch_id * 2 + price_limit', f'batch_id * 2 + price_limit + {variant}')
    return WEIGHTED.replace('quote.unit_price_cents <= price_limit',
                            f'quote.unit_price_cents <= price_limit - {variant * 17}')


def oracle(case, rows, variant):
    if case == 'scalar':
        v = 734 + variant
        return f'{v}\nTrue\n'
    total = sum(((i * 37) % 1000) * (i % 9 - 2) for i in range(rows)
                if i % 9 - 2 > 0 and (i * 37) % 1000 <= 700 - variant * 17)
    return f'{total + 17}\n'


def worker(arm):
    argv = [str(PYTHON), '-I', str(PY_IMPL), '--fork-server'] if arm == 'prefork' else [str(BINARY), arm]
    begin = time.perf_counter_ns()
    process = subprocess.Popen(argv, cwd=ROOT, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, bufsize=0)
    assert process.stdout is not None
    assert json.loads(process.stdout.readline()) == {'ready': True}
    return process, time.perf_counter_ns() - begin


def request(arm, process, payload):
    begin = time.perf_counter_ns()
    if arm == 'spawn':
        cp = subprocess.run([str(PYTHON), '-I', str(PY_IMPL), '--once'], input=payload,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, cwd=ROOT, timeout=120)
        if cp.returncode != 0:
            raise RuntimeError(f'fresh Python exit {cp.returncode}: {cp.stderr[:500]!r}')
        line = cp.stdout
    else:
        process.stdin.write(payload)
        process.stdin.flush()
        line = process.stdout.readline()
        if not line:
            raise RuntimeError(f'{arm} exited {process.poll()}: {process.stderr.read()[:500]!r}')
    response = json.loads(line)
    elapsed = time.perf_counter_ns() - begin
    if 'error' in response:
        raise RuntimeError(f'{arm}: {response["error"]}')
    return elapsed, response['ok']


def schedule(smoke):
    for round_ in range(1 if smoke else 3):
        for case, rows in SHAPES if not smoke else [('scalar', 0), ('weighted', 100), ('weighted', 100000)]:
            for sample in range(1 if smoke else 10):
                variant = round_ * 10 + sample
                req = dict(source=source(case, variant), rows=rows)
                payload = (json.dumps(req, separators=(',', ':')) + '\n').encode()
                order = ARMS[(sample + round_) % 5:] + ARMS[:(sample + round_) % 5]
                for arm in order:
                    yield round_, case, rows, sample, variant, arm, payload


def campaign(out, smoke=False):
    out.mkdir(parents=True, exist_ok=True)
    raw = out / ('smoke.jsonl' if smoke else 'raw.jsonl')
    if raw.exists():
        raise ValueError(f'raw already exists: {raw}')
    count = 0
    current_round = None
    processes = {}
    startup = []
    with raw.open('w') as file:
        try:
            for round_, case, rows, sample, variant, arm, payload in schedule(smoke):
                if round_ != current_round:
                    assert not processes or all(p.poll() is not None for p in processes.values())
                    started = {a: worker(a) for a in ARMS if a != 'spawn'}
                    processes = {a: item[0] for a, item in started.items()}
                    startup.extend(dict(round=round_, arm=a, startup_ns=item[1]) for a, item in started.items())
                    current_round = round_
                row = dict(round=round_, case=case, rows=rows, sample=sample,
                           variant=variant, arm=arm, status='failed')
                try:
                    elapsed, result = request(arm, processes.get(arm), payload)
                    assert result == dict(stdout=oracle(case, rows, variant),
                                          loads=int(case != 'scalar'), records=int(case != 'scalar')), result
                    row.update(status='completed', wall_ns=elapsed, **result)
                except Exception as exc:
                    row['error'] = repr(exc)
                file.write(json.dumps(row) + '\n')
                file.flush()
                count += 1
                if row['status'] != 'completed':
                    raise RuntimeError(row)
                # End of a round after its last arm and shape.
                last_arm = ARMS[((sample + round_ - 1) % 5)]
                if not smoke and case == 'weighted' and rows == 100000 and sample == 9 and arm == last_arm:
                    for p in processes.values():
                        assert p.stdin is not None and p.stderr is not None
                        p.stdin.close()
                        p.wait(timeout=10)
                        assert p.returncode == 0, p.stderr.read()
                if smoke and case == 'weighted' and rows == 100000 and arm == last_arm:
                    for p in processes.values():
                        assert p.stdin is not None and p.stderr is not None
                        p.stdin.close(); p.wait(timeout=10)
                        assert p.returncode == 0, p.stderr.read()
        finally:
            for p in processes.values():
                if p.poll() is None:
                    p.kill(); p.wait()
    print(f'{raw}: {count} successful requests', flush=True)
    (out / ('smoke-startup.json' if smoke else 'startup.json')).write_text(json.dumps(startup, indent=2) + '\n')
    return raw


def summarize(raw):
    rows = [json.loads(line) for line in raw.read_text().splitlines()]
    assert len(rows) == 3 * len(SHAPES) * 10 * len(ARMS)
    assert all(row['status'] == 'completed' for row in rows)
    assert all(r['stdout'] == oracle(r['case'], r['rows'], r['variant']) and
               (r['loads'], r['records']) == ((0, 0) if r['case'] == 'scalar' else (1, 1)) for r in rows)
    assert len({(r['round'], r['case'], r['rows'], r['sample'], r['arm']) for r in rows}) == len(rows)
    assert all(r['variant'] == r['round'] * 10 + r['sample'] for r in rows)
    for rnd in range(3):
        for case, n in SHAPES:
            for sample in range(10):
                assert {r['arm'] for r in rows if (r['round'], r['case'], r['rows'], r['sample']) == (rnd, case, n, sample)} == set(ARMS)
    cells = []
    for case, n in SHAPES:
        for arm in ARMS:
            values = [r['wall_ns'] / 1000 for r in rows if (r['case'], r['rows'], r['arm']) == (case, n, arm)]
            assert len(values) == 30
            by_round = [statistics.mean(values[i * 10:(i + 1) * 10]) for i in range(3)]
            cells.append(dict(case=case, rows=n, arm=arm, count=30,
                              mean_us=statistics.mean(values), median_us=statistics.median(values),
                              min_us=min(values), max_us=max(values), round_mean_us=by_round))
    return cells


if __name__ == '__main__':
    out = Path(sys.argv[1]).resolve()
    smoke = sys.argv[2:] == ['--smoke']
    raw = campaign(out, smoke)
    if not smoke:
        (out / 'summary.json').write_text(json.dumps(summarize(raw), indent=2) + '\n')
