"""S request worker: frozen L Python path with byte-identical common bodies.
Usage: PYTHON tools/s_wsl_python.py speed JSON | memory CODE COUNT
Works under CPython and PyPy. Process startup is outside resident observations.
"""
import asyncio
import gc
import json
from pathlib import Path
import sys
import time
import final_python as base

ROOT = Path(__file__).resolve().parents[1]
BODIES = {c: (ROOT / ('fixtures/common_' + c + '.py')).read_text()
          for c in ['scalar', 'weighted', 'stateful']}


async def speed(config):
    config['bodies'] = BODIES
    label = config.pop('label')
    start = time.perf_counter_ns()
    result = await base.benchmark_embedded(config)
    for r in result:
        r.update(engine=label, corpus='common', version=sys.version,
                 command_elapsed_ns=time.perf_counter_ns() - start)
    return result


async def memory(code_mode, tasks):
    assert code_mode in ['unique', 'shared'] and 1 <= tasks <= 32
    try:
        import tracemalloc
        tracemalloc.start()
    except (ImportError, NotImplementedError):
        tracemalloc = None
    gc.collect()
    rows = []
    def snap(phase):
        rows.append(dict(kind='hold', engine=sys.implementation.name + '-resident',
                         code=code_mode, count=tasks, rows=10000, phase=phase,
                         requested_traced_bytes=tracemalloc.get_traced_memory()[0] if tracemalloc else None,
                         physical=base.physical([__import__('os').getpid()]), corpus='common'))
    snap('startup')
    shared = base.prepare('weighted', BODIES['weighted']) if code_mode == 'shared' else None
    load, record = asyncio.Event(), asyncio.Event()
    providers = [base.Provider(10000, 0, load, record, common=True) for _ in range(tasks)]
    async def task(p):
        code = shared if shared is not None else base.prepare('weighted', BODIES['weighted'])
        output = []
        scope = {'print': lambda v: output.append(str(v) + '\n')}
        exec(code, scope)
        await scope['program'](p, 17, 700)
        scope.clear()
        assert ''.join(output) == base.oracle('weighted', 10000)
    jobs = [asyncio.create_task(task(p)) for p in providers]
    for phase, counter, gate in [('load_wait', 'loads', load), ('record_wait', 'records', record)]:
        while not all(getattr(p, counter) == 1 for p in providers):
            await asyncio.sleep(0)
        snap(phase)
        gate.set()
    await asyncio.gather(*jobs)
    del jobs, providers, shared
    gc.collect()
    snap('reclaimed')
    if tracemalloc:
        tracemalloc.stop()
    return rows


if __name__ == '__main__':
    rows = asyncio.run(speed(json.loads(sys.argv[2]))) if sys.argv[1] == 'speed' else asyncio.run(memory(sys.argv[2], int(sys.argv[3])))
    for r in rows:
        print(json.dumps(r))
