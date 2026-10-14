"""AD single-threaded import-prewarmed parent. Each fork request exits and is reaped.
No compiled user code cache. Tools cross a real TCP/JSON boundary to the same Go
producer as Pysolate. This is a controlled transport, not a production sandbox.
"""
import asyncio
import contextlib
import gc
import io
import json
import os
import resource
import socket
import sys
from types import SimpleNamespace


def tool(address, name, args):
    with socket.create_connection(tuple(address), timeout=600) as conn:
        f = conn.makefile('rwb')
        f.write(json.dumps({'tool': name, 'args': args}, separators=(',', ':')).encode() + b'\n')
        f.flush()
        result = json.loads(f.readline())
        if 'error' in result:
            raise RuntimeError(result['error'])
        return result['value']


def execute(source, inputs, address, empty):
    if empty:
        tool(address, 'empty', {})
    tools = SimpleNamespace(
        load_quotes=lambda **args: tool(address, 'load', args),
        record_total=lambda **args: tool(address, 'record', args))
    scope = {'inputs': inputs, 'tools': tools}
    out = io.StringIO()
    try:
        with contextlib.redirect_stdout(out):
            exec(compile(source, 'common.py', 'exec'), scope)
        return {'stdout': out.getvalue()}
    finally:
        scope.clear()


async def execute_resident(source, inputs, address):
    async def call(name, args):
        reader, writer = await asyncio.open_connection(*address, limit=8 << 20)
        writer.write(json.dumps({'tool': name, 'args': args}).encode() + b'\n')
        await writer.drain()
        try:
            result = json.loads(await reader.readline())
        finally:
            writer.close()
            await writer.wait_closed()
        if 'error' in result: raise RuntimeError(result['error'])
        return result['value']
    out = []
    scope = {'inputs': inputs,
             'tools': SimpleNamespace(load_quotes=lambda **a: call('load', a),
                                      record_total=lambda **a: call('record', a)),
             'print': lambda v: out.append(str(v) + '\n')}
    try:
        exec(compile(source, 'common.py', 'exec'), scope)
        await scope['program']()
        return {'stdout': ''.join(out)}
    finally:
        scope.clear()


def cpu():
    return sum(round((r.ru_utime + r.ru_stime) * 1e9) for r in
               [resource.getrusage(resource.RUSAGE_SELF), resource.getrusage(resource.RUSAGE_CHILDREN)])


async def main(arm, host, port):
    sources = {}
    empty = False
    children = set()
    async def handler(reader, writer):
        nonlocal empty
        try:
            req = json.loads(await reader.readline())
            op = req['op']
            if op == 'cpu':
                result = {'cpu_ns': cpu(), 'children': len(children)}
            elif op == 'sources':
                bodies = req['sources']
                if arm == 'resident':
                    bodies = {c: 'async def program():\n' + ''.join('    ' + line + '\n' for line in s.replace('tools.load_quotes(', 'await tools.load_quotes(').replace('tools.record_total(', 'await tools.record_total(').splitlines()) for c, s in bodies.items()}
                sources.update(bodies)
                # Freeze only import-prewarmed parent state, never user code.
                gc.collect(); gc.freeze()
                result = {'ready': True}
            elif op == 'empty':
                empty = req['enabled']
                result = {'ready': True}
            elif op == 'run':
                if arm == 'resident':
                    result = await execute_resident(sources[req['case']], req['inputs'], (host, port))
                else:
                    read_fd, write_fd = os.pipe()
                    pid = os.fork()
                    if pid == 0:
                        os.close(read_fd)
                        # Close inherited listeners/control peers. Keep only stdin/out/err
                        # and our result pipe. No live event-loop marker enters execution.
                        keep = {0, 1, 2, write_fd}
                        for fd in list(os.listdir('/proc/self/fd')):
                            if int(fd) not in keep:
                                try: os.close(int(fd))
                                except OSError: pass
                        try:
                            result = execute(sources[req['case']], req['inputs'], (host, port), empty)
                            with os.fdopen(write_fd, 'w') as f:
                                json.dump(result, f)
                            os._exit(0)
                        except BaseException as exc:
                            with os.fdopen(write_fd, 'w') as f:
                                json.dump({'error': repr(exc)}, f)
                            os._exit(1)
                    os.close(write_fd)
                    children.add(pid)
                    child_reader = asyncio.StreamReader()
                    protocol = asyncio.StreamReaderProtocol(child_reader)
                    transport, _ = await asyncio.get_running_loop().connect_read_pipe(
                        lambda: protocol, os.fdopen(read_fd, 'rb', buffering=0))
                    try:
                        payload = await child_reader.read()
                    finally:
                        transport.close()
                    _, status = os.waitpid(pid, 0)
                    children.remove(pid)
                    result = json.loads(payload)
                    if status != 0:
                        result = {'error': result.get('error', f'child status {status}')}
            else:
                raise ValueError(op)
        except BaseException as exc:
            result = {'error': repr(exc)}
        writer.write(json.dumps(result).encode() + b'\n')
        await writer.drain()
        writer.close()
        await writer.wait_closed()
    server = await asyncio.start_server(handler, '127.0.0.1', 0, limit=1 << 20)
    print(json.dumps({'port': server.sockets[0].getsockname()[1], 'pid': os.getpid(), 'version': sys.version}), flush=True)
    async with server:
        await server.serve_forever()


if __name__ == '__main__':
    asyncio.run(main(sys.argv[1], sys.argv[2], int(sys.argv[3])))
