"""BC CPython arms: identical request implementation for fresh and prewarmed fork."""
import array
import asyncio
from collections import namedtuple
import json
import os
import sys

Quote = namedtuple('Quote', 'unit_price_cents quantity')

class Tools:
    def __init__(self, rows):
        self.rows = rows
        self.loads = self.records = 0

    async def load_quotes(self, batch_id):
        assert batch_id == 17
        self.loads += 1
        packed = array.array('q', (x for i in range(self.rows)
                                   for x in ((i * 37) % 1000, i % 9 - 2)))
        result = [Quote(packed[i], packed[i + 1]) for i in range(0, len(packed), 2)]
        return result

    async def record_total(self, batch_id, total):
        assert batch_id == 17
        self.records += 1
        return total + batch_id


def execute(req):
    source = req['source']
    assert isinstance(source, str) and 0 <= req['rows'] <= 100000
    wrapper = 'async def program(tools, batch_id, price_limit):\n' + ''.join(
        '    ' + line for line in source.splitlines(keepends=True))
    code = compile(wrapper, 'common.py', 'exec')
    tools = Tools(req['rows'])
    out = []
    scope = {'print': lambda value: out.append(str(value) + '\n')}
    try:
        exec(code, scope)
        asyncio.run(scope['program'](tools, 17, 700))
    finally:
        scope.clear()
    return {'stdout': ''.join(out), 'loads': tools.loads, 'records': tools.records}


def result(req):
    try:
        return {'ok': execute(req)}
    except BaseException as exc:
        return {'error': repr(exc)}


def serve_fork():
    # Parent imports shared dependencies but never compiles user source or builds input.
    print(json.dumps({'ready': True}), flush=True)
    for line in sys.stdin:
        req = json.loads(line)
        read_fd, write_fd = os.pipe()
        pid = os.fork()
        if pid == 0:
            os.close(read_fd)
            try:
                with os.fdopen(write_fd, 'w') as pipe:
                    pipe.write(json.dumps(result(req)) + '\n')
                os._exit(0)
            except BaseException:
                os._exit(1)
        os.close(write_fd)
        with os.fdopen(read_fd) as pipe:
            reply = pipe.read()
        _, status = os.waitpid(pid, 0)
        if status != 0:
            reply = json.dumps({'error': f'child status {status}'}) + '\n'
        sys.stdout.write(reply)
        sys.stdout.flush()


if __name__ == '__main__':
    if sys.argv[1:] == ['--once']:
        print(json.dumps(result(json.loads(sys.stdin.readline()))), flush=True)
    elif sys.argv[1:] == ['--fork-server']:
        serve_fork()
    else:
        raise SystemExit('usage: bc_python.py --once|--fork-server')
