"""L ordinary CPython/Pyodide source execution, not a handwritten scan helper.
Worker-generated signed-64 array -> list-of-tuples is fully inside execution.
Python uses ordinary arbitrary-precision arithmetic/blocking scan turns; this
is success-corpus parity, NOT MiniPy authority/fuel/cancellation equivalence.
CPython CLI: final_python.py campaign OUT; or worker (one JSON request on stdin).
Pyodide loads this same file, calls benchmark_embedded(config), no subprocess.
"""
import array
import asyncio
import gc
import json
import os
from pathlib import Path
import sys
import time
from collections import namedtuple

QuoteValue = namedtuple('Quote', ['unit_price_cents', 'quantity'])

SOURCES = {
    'scalar': 'value = batch_id * 2 + price_limit\nprint(value)\nprint(value > 0)\nvalue > 0',
    'weighted': 'quotes = await tools.load_quotes(batch_id)\ntotal = 0\nfor quote in quotes:\n    if quote[1] > 0 and quote[0] <= price_limit:\n        amount = quote[0] * quote[1]\n        total += amount\nreceipt = await tools.record_total(batch_id, total)\nprint(receipt)',
    'stateful': 'quotes = await tools.load_quotes(batch_id)\ntotal = 0\nprevious = 0\nfor quote in quotes:\n    if quote[1] > 0 and quote[0] <= price_limit and quote[0] >= previous:\n        amount = quote[0] * quote[1]\n        total += amount\n        previous = quote[0]\nreceipt = await tools.record_total(batch_id, total)\nprint(receipt)',
}
SHAPES = [('scalar', 0), ('weighted', 100), ('stateful', 100),
          ('weighted', 10000), ('stateful', 100000)]
WEIGHTS = {'light': [24, 8, 4, 2, 2], 'scan': [8, 16, 8, 4, 4]}


def oracle(case, n):
    if case == 'scalar':
        return '734\nTrue\n'
    total = previous = 0
    for i in range(n):
        price, quantity = (i * 37) % 1000, i % 9 - 2
        if quantity > 0 and price <= 700 and (case != 'stateful' or price >= previous):
            total += price * quantity
            if case == 'stateful': previous = price
    return str(total + 17) + '\n'


def prepare(case, body=None):
    if body is not None:
        assert body.endswith('\n')
        wrapper = 'async def program(tools, batch_id, price_limit):\n' + ''.join('    ' + line for line in body.splitlines(keepends=True))
        assert ''.join(line[4:] for line in wrapper.splitlines(keepends=True)[1:]) == body
        return compile(wrapper, 'common.py', 'exec')
    return compile('async def program(tools, batch_id, price_limit):\n' +
                   ''.join('    ' + line + '\n' for line in SOURCES[case].splitlines()),
                   'fixture.py', 'exec')


class Provider:
    def __init__(self, n, delay_us, load=None, record=None, common=False):
        self.n, self.delay = n, delay_us / 1e6
        self.load, self.record = load, record
        self.common = common
        self.loads = self.records = 0
        self.conversion_ns = 0

    async def load_quotes(self, batch):
        assert batch == 17
        self.loads += 1
        start = time.perf_counter_ns()
        packed = array.array('q', (v for i in range(self.n)
                                  for v in ((i * 37) % 1000, i % 9 - 2)))
        self.conversion_ns += time.perf_counter_ns() - start
        if self.load is not None: await self.load.wait()
        await asyncio.sleep(self.delay)
        start = time.perf_counter_ns()
        result = [QuoteValue(packed[i], packed[i + 1]) for i in range(0, len(packed), 2)] if self.common else [(packed[i], packed[i + 1]) for i in range(0, len(packed), 2)]
        del packed
        self.conversion_ns += time.perf_counter_ns() - start
        return result

    async def record_total(self, batch, total):
        assert batch == 17 and -(1 << 63) <= total < (1 << 63) - 17
        self.records += 1
        if self.record is not None: await self.record.wait()
        await asyncio.sleep(self.delay)
        return total + batch


async def execute(config, code=None, gates=None):
    start = time.perf_counter_ns()
    p = Provider(config['rows'], config.get('delay_us', 0), *(gates or (None, None)), common='body' in config)
    out = []
    prep_start = time.perf_counter_ns()
    code = prepare(config['case'], config.get('body')) if code is None else code
    prepare_ns = time.perf_counter_ns() - prep_start
    scope = {'print': lambda value: out.append(str(value) + '\n')}
    exec(code, scope)
    await scope['program'](p, 17, 700)
    # Remove the exec namespace/function cycle; include its cleanup in execution.
    scope.clear()
    result = ''.join(out)
    return {'stdout': result, 'prepare_ns': prepare_ns,
            'conversion_ns': p.conversion_ns,
            'worker_wall_ns': time.perf_counter_ns() - start,
            'loads': p.loads, 'records': p.records}


def worker(config):
    return asyncio.run(execute(config))


def total_cpu_ns():
    import resource
    def ns(who):
        r = resource.getrusage(who)
        return int((r.ru_utime + r.ru_stime) * 1e9)
    return ns(resource.RUSAGE_SELF) + ns(resource.RUSAGE_CHILDREN)


def embedded_cpu_ns():
    from js import bench_cpu_ns
    return int(bench_cpu_ns())


async def isolated(config, mode):
    if mode == 'cpython-spawn':
        proc = await asyncio.create_subprocess_exec(
            sys.executable, '-I', str(Path(__file__).resolve()), 'worker',
            stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE)
        stdout, stderr = await proc.communicate((json.dumps(config) + '\n').encode())
        assert proc.returncode == 0, stderr.decode()
        return json.loads(stdout)
    # Fork only the single-threaded, import-prewarmed controller. Child creates
    # its own event loop; no inherited active loop or source-code cache is used.
    read_fd, write_fd = os.pipe()
    pid = os.fork()
    if pid == 0:
        os.close(read_fd)
        try:
            asyncio.set_event_loop(None)
            # asyncio.run checks the thread-local running-loop marker after fork.
            asyncio.events._set_running_loop(None)
            result = worker(config)
            with os.fdopen(write_fd, 'w') as f: f.write(json.dumps(result))
            os._exit(0)
        except BaseException:
            os._exit(1)
    os.close(write_fd)
    reader = asyncio.StreamReader()
    protocol = asyncio.StreamReaderProtocol(reader)
    transport, _ = await asyncio.get_running_loop().connect_read_pipe(
        lambda: protocol, os.fdopen(read_fd, 'rb', buffering=0))
    try:
        payload = await reader.read()
    finally:
        transport.close()
    _, status = os.waitpid(pid, 0)
    assert status == 0, status
    return json.loads(payload)


async def benchmark_embedded(config):
    """Per-command raw observations; Node owns outer process CPU/init metrics."""
    engine = config['engine']
    embedded = engine.startswith('pyodide')
    prepared = engine in ['cpython-prepared', 'pyodide-prepared']
    bodies = config.get('bodies', {})
    cached = {case: prepare(case, bodies.get(case)) for case, _ in SHAPES} if prepared else {}
    expected = {tuple(shape): oracle(*shape) for shape in SHAPES}
    emitted = []
    async def run_one(case, n, delay):
        cfg = {'case': case, 'rows': n, 'delay_us': delay}
        if case in bodies: cfg['body'] = bodies[case]
        if engine in ['cpython-spawn', 'cpython-fork']:
            return await isolated(cfg, engine)
        return await execute(cfg, cached.get(case))
    for sample in range(config['reps']):
        if config['kind'] == 'speed':
            case, n = config['case'], config['rows']
            want = oracle(case, n)
            cpu = embedded_cpu_ns() if embedded else total_cpu_ns()
            start = time.perf_counter_ns()
            result = await run_one(case, n, config.get('delay_us', 0))
            wall = time.perf_counter_ns() - start
            cpu = (embedded_cpu_ns() if embedded else total_cpu_ns()) - cpu
            assert result['stdout'] == want
            assert (result['loads'], result['records']) == ((0, 0) if case == 'scalar' else (1, 1))
            emitted.append(dict(result, kind='speed', engine=engine, case=case, rows=n,
                                sample=sample, wall_ns=wall, cpu_ns=cpu, completed=True))
        else:
            schedule = [i for i, count in enumerate(WEIGHTS[config['mix']]) for _ in range(count)]
            sem = asyncio.Semaphore(config.get('inflight', 40))
            cpu = embedded_cpu_ns() if embedded else total_cpu_ns()
            start = time.perf_counter_ns()
            async def request(arrival):
                case, n = SHAPES[schedule[(arrival * 17) % 40]]
                due = start + arrival * 500_000
                await asyncio.sleep(max(0, (due - time.perf_counter_ns()) / 1e9))
                async with sem:
                    result = await run_one(case, n, config['delay_us'])
                latency = time.perf_counter_ns() - due
                assert result['stdout'] == expected[(case, n)]
                assert (result['loads'], result['records']) == ((0, 0) if case == 'scalar' else (1, 1))
                return dict(result, kind='mixed', engine=engine, mix=config['mix'],
                            delay_us=config['delay_us'], inflight=config.get('inflight', 40),
                            sample=sample, arrival=arrival, case=case, rows=n,
                            latency_ns=latency, completed=True)
            rows = await asyncio.gather(*(request(i) for i in range(40)))
            wall = time.perf_counter_ns() - start
            cpu = (embedded_cpu_ns() if embedded else total_cpu_ns()) - cpu
            for r in rows: r.update(batch_wall_ns=wall, batch_cpu_ns=cpu, overloaded=wall > 100_000_000)
            emitted.extend(rows)
            await asyncio.sleep(max(0, (start + 100_000_000 - time.perf_counter_ns()) / 1e9))
    return emitted


def physical(pids):
    if sys.platform == 'emscripten':
        from js import process_snapshot
        return json.loads(process_snapshot())
    rss = pss = 0
    per_pid = []
    for pid in pids:
        text = Path(f'/proc/{pid}/smaps_rollup').read_text()
        values = {line.split(':')[0]: int(line.split()[1]) * 1024
                  for line in text.splitlines() if line.startswith(('Rss:', 'Pss:'))}
        rss += values['Rss']; pss += values['Pss']
        per_pid.append(dict(pid=pid, **values))
    return {'rss_bytes': rss, 'pss_bytes': pss, 'per_pid': per_pid}


async def memory_resident(engine, code_mode, tasks=8):
    try:
        import tracemalloc
        tracemalloc.start()
    except (ImportError, NotImplementedError):
        tracemalloc = None
    gc.collect()
    rows = []
    def snap(phase):
        rows.append({'kind': 'secondary-hold', 'engine': engine, 'code': code_mode,
                     'count': tasks, 'rows': 10000, 'phase': phase,
                     'requested_traced_bytes': tracemalloc.get_traced_memory()[0] if tracemalloc else None,
                     'physical': physical([os.getpid()])})
    snap('startup')
    shared = prepare('weighted') if code_mode == 'shared' else None
    load, record = asyncio.Event(), asyncio.Event()
    providers = [Provider(10000, 0, load, record) for _ in range(tasks)]
    # Direct source execution with per-request print, same as speed path.
    async def task(p):
        code = shared if shared is not None else prepare('weighted')
        out = []
        scope = {'print': lambda value: out.append(str(value) + '\n')}
        exec(code, scope)
        await scope['program'](p, 17, 700)
        scope.clear()
        assert ''.join(out) == oracle('weighted', 10000)
    jobs = [asyncio.create_task(task(p)) for p in providers]
    for phase, counter, gate in [('load_wait', 'loads', load), ('record_wait', 'records', record)]:
        while not all(getattr(p, counter) == 1 for p in providers): await asyncio.sleep(0)
        snap(phase)
        gate.set()
    await asyncio.gather(*jobs)
    del jobs, providers, shared
    gc.collect()
    snap('reclaimed')
    if tracemalloc: tracemalloc.stop()
    return rows


def campaign(out):
    out = Path(out); out.mkdir(exist_ok=True)
    raw = out / 'cpython.jsonl.gz'
    assert not raw.exists(), 'preserve raw'
    import gzip
    import platform
    engines = ['cpython-spawn', 'cpython-resident', 'cpython-prepared', 'cpython-fork']
    commands, count = [], 0
    # Run a real smoke through all modes before timed formal observations.
    for engine in engines:
        assert asyncio.run(benchmark_embedded({'engine': engine, 'kind': 'speed',
                      'case': 'weighted', 'rows': 100, 'reps': 1}))[0]['completed']
    with gzip.open(raw, 'wt') as f:
        for ri in range(3):
            order = engines[ri:] + engines[:ri]
            if ri % 2: order.reverse()
            for case, n in SHAPES:
                for engine in order:
                    cfg = dict(engine=engine, kind='speed', case=case, rows=n, reps=21)
                    commands.append(dict(round=ri, **cfg))
                    for r in asyncio.run(benchmark_embedded(cfg)):
                        r['round'] = ri; f.write(json.dumps(r) + '\n'); count += 1
                    f.flush()
            for mix in ['light', 'scan']:
                for delay in [0, 1000]:
                    for engine in order:
                        cfg = dict(engine=engine, kind='mixed', mix=mix, delay_us=delay,
                                   inflight=40, reps=5)
                        commands.append(dict(round=ri, **cfg))
                        for r in asyncio.run(benchmark_embedded(cfg)):
                            r['round'] = ri; f.write(json.dumps(r) + '\n'); count += 1
                        f.flush()
            print('CPython round', ri, 'complete', count, flush=True)
    assert count == 3 * 4 * (5 * 21 + 4 * 5 * 40) == 10860
    (out / 'cpython-identity.json').write_text(json.dumps({
        'version': sys.version, 'executable': sys.executable, 'platform': platform.platform(),
        'cpu_accounting': 'getrusage SELF + reaped CHILDREN; no double-counted child self-report',
        'commands': commands, 'raw_count': count, 'completed_commands': len(commands),
        'source': SOURCES, 'setup': 'controller imports excluded from steady-process rows; spawn includes full child interpreter/import/init and teardown; fork includes fork, child loop and teardown; resident and prepared keep interpreter process alive',
        'differences': 'ordinary Python ints, tuple objects, blocking scans, asyncio sleeps, no MiniPy fuel/capabilities; bounded successful corpus only'
    }, indent=2) + '\n')
    # Fresh diagnostic process per cell avoids allocator/history contamination.
    import subprocess
    with (out / 'cpython-memory.jsonl').open('w') as f:
        for ri in range(3):
            for code_mode in ['unique', 'shared']:
                result = subprocess.check_output([sys.executable, '-I', __file__, 'memory', code_mode], text=True)
                for line in result.splitlines():
                    r = json.loads(line); r['round'] = ri; f.write(json.dumps(r) + '\n')
            for engine in ['cpython-spawn', 'cpython-fork']:
                result = subprocess.check_output([sys.executable, '-I', __file__, 'tree-memory', engine], text=True)
                for line in result.splitlines():
                    r = json.loads(line); r['round'] = ri; f.write(json.dumps(r) + '\n')


def hold_worker():
    class PipeGate:
        def __init__(self, phase): self.phase = phase
        async def wait(self):
            print(json.dumps({'phase': self.phase}), flush=True)
            assert sys.stdin.readline().strip() == 'release'
    result = asyncio.run(execute({'case': 'weighted', 'rows': 10000},
                         gates=(PipeGate('load_wait'), PipeGate('record_wait'))))
    assert result['stdout'] == oracle('weighted', 10000)
    print(json.dumps({'phase': 'done'}), flush=True)


def common_campaign(out, body_file):
    import gzip
    bodies = json.loads(Path(body_file).read_text())
    for case in SOURCES: prepare(case, bodies[case]) # checks wrapper round trip
    out = Path(out)
    raw = out / 'common-cpython.jsonl.gz'
    assert not raw.exists(), 'preserve raw'
    engines = ['cpython-resident','cpython-prepared']
    count = 0
    commands = []
    with gzip.open(raw,'wt') as f:
        for ri in range(3):
            order = engines if ri % 2 == 0 else engines[::-1]
            for case,n in SHAPES:
                for engine in order:
                    cfg = dict(engine=engine,kind='speed',case=case,rows=n,reps=21,bodies=bodies)
                    commands.append(dict(round=ri,**cfg))
                    for r in asyncio.run(benchmark_embedded(cfg)):
                        r.update(round=ri,corpus='common'); f.write(json.dumps(r)+'\n'); count+=1
                    f.flush()
            for mix in ['light','scan']:
                for delay in [0,1000]:
                    for engine in order:
                        cfg = dict(engine=engine,kind='mixed',mix=mix,delay_us=delay,inflight=40,reps=5,bodies=bodies)
                        commands.append(dict(round=ri,**cfg))
                        for r in asyncio.run(benchmark_embedded(cfg)):
                            r.update(round=ri,corpus='common'); f.write(json.dumps(r)+'\n'); count+=1
                        f.flush()
            print('common CPython round',ri,'complete',count,flush=True)
    assert count == 5430
    (out/'common-cpython-identity.json').write_text(json.dumps({'version':sys.version,
        'commands':commands,'raw_count':count,'body':bodies,
        'wrappers':{case:'async def program(tools, batch_id, price_limit):\n'+''.join('    '+l for l in body.splitlines(keepends=True)) for case,body in bodies.items()}},indent=2)+'\n')


def tree_memory(engine):
    """Include controller and 8 living children; PSS sums shared pages once."""
    import subprocess
    children = []
    rows = []
    def snap(phase):
        rows.append({'kind': 'secondary-hold', 'engine': engine, 'code': 'unique',
                     'count': 8, 'rows': 10000, 'phase': phase,
                     'requested_traced_bytes': None,
                     'physical': physical([os.getpid()] + [p[0] for p in children])})
    snap('startup')
    for _ in range(8):
        if engine == 'cpython-spawn':
            p = subprocess.Popen([sys.executable, '-I', __file__, 'hold-worker'],
                                 stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
            children.append((p.pid, p.stdin, p.stdout, p))
        else:
            parent_read, child_write = os.pipe()
            child_read, parent_write = os.pipe()
            pid = os.fork()
            if pid == 0:
                os.close(parent_read); os.close(parent_write)
                sys.stdin = os.fdopen(child_read, 'r')
                sys.stdout = os.fdopen(child_write, 'w')
                hold_worker()
                os._exit(0)
            os.close(child_read); os.close(child_write)
            children.append((pid, os.fdopen(parent_write, 'w'), os.fdopen(parent_read, 'r'), None))
    for phase in ['load_wait', 'record_wait']:
        for _, _, reader, _ in children: assert json.loads(reader.readline())['phase'] == phase
        snap(phase)
        for _, writer, _, _ in children: writer.write('release\n'); writer.flush()
    for pid, writer, reader, p in children:
        assert json.loads(reader.readline())['phase'] == 'done'
        writer.close(); reader.close()
        if p is not None: assert p.wait() == 0
        else: assert os.waitpid(pid, 0)[1] == 0
    children.clear()
    snap('reclaimed')
    for r in rows: print(json.dumps(r))


if __name__ == '__main__':
    if sys.argv[1] == 'worker':
        print(json.dumps(worker(json.loads(sys.stdin.readline()))))
    elif sys.argv[1] == 'memory':
        for row in asyncio.run(memory_resident('cpython-resident', sys.argv[2])): print(json.dumps(row))
    elif sys.argv[1] == 'campaign':
        campaign(sys.argv[2])
    elif sys.argv[1] == 'hold-worker':
        hold_worker()
    elif sys.argv[1] == 'common-campaign':
        common_campaign(sys.argv[2], sys.argv[3])
    elif sys.argv[1] == 'tree-memory':
        tree_memory(sys.argv[2])
