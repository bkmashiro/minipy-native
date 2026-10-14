"""One fresh ordinary Python process and its first common-body request.
Absolute perf_counter_ns boundaries share Linux CLOCK_MONOTONIC with controller.
No warmup/prepare probe precedes the measured source preparation.
"""
import time
INIT_BEGIN = time.perf_counter_ns()
import asyncio
import json
from pathlib import Path
import sys
import final_python as base


def first(case, rows):
    body = (Path(__file__).resolve().parents[1] / ('fixtures/common_' + case + '.py')).read_text()
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    ready = time.perf_counter_ns()
    code = base.prepare(case, body)
    prepared = time.perf_counter_ns()
    result = loop.run_until_complete(base.execute(dict(case=case, rows=rows, body=body), code))
    executed = time.perf_counter_ns()
    assert result['stdout'] == base.oracle(case, rows)
    loop.close()
    return dict(kind='first-request', engine=sys.implementation.name, case=case, rows=rows,
                init_begin_ns=INIT_BEGIN, ready_ns=ready, prepared_ns=prepared, executed_ns=executed,
                stdout=result['stdout'], completed=True,
                prepare_boundary='common body async wrapper compilation',
                execute_boundary='scope exec, generated input, object conversion, tools, output and scope cleanup')


if __name__ == '__main__':
    print(json.dumps(first(sys.argv[1], int(sys.argv[2]))))
