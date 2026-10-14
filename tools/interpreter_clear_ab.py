"""Secondary G control: ordinary Monty explicitly releases its last-use list.
Usage identical to interpreter_ab.py. Does not rerun the primary matrix.
"""
import gzip
import hashlib
import json
from pathlib import Path
import shlex
import subprocess
import sys
ROOT = Path(__file__).resolve().parents[1]
def run(args):
    return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT)
def main():
    host, key, known = sys.argv[1:]
    opts = ['-i', key, '-o', f'UserKnownHostsFile={known}', '-o', 'StrictHostKeyChecking=yes', '-o', 'BatchMode=yes']
    ssh = ['ssh', *opts, host]; scp = ['scp', *opts]
    remote = '/root/minipy-native-g-interpreter'; out = ROOT / 'bench/results/g-interpreter'
    assert not (out / 'clear-linux.jsonl.gz').exists()
    identity = {'purpose': 'strong ordinary Monty list-release control; same script computation and real Tools, quotes=[] inserted before terminal record, safe only for fixed benchmark shapes', 'preflight': run(ssh + ['uname -a; uptime; free -m; ps']), 'binaries': {}, 'commands': [], 'harness_sha256': hashlib.sha256((ROOT / 'bench/src/bin/interpreter_ab.rs').read_bytes()).hexdigest()}
    for mode in ['speed', 'memory']:
        p = ROOT / f'target/g-clear-{mode}-linux'; sha = hashlib.sha256(p.read_bytes()).hexdigest()
        run(scp + [str(p), f'{host}:{remote}/clear-{mode}'])
        assert run(ssh + [f'sha256sum {remote}/clear-{mode}']).split()[0] == sha
        identity['binaries'][mode] = {'sha256': sha, 'bytes': p.stat().st_size}
    with gzip.open(out / 'clear-linux.jsonl.gz', 'wt') as f:
        for round_id in range(3):
            engines = ['monty', 'monty-clear'] if round_id % 2 == 0 else ['monty-clear', 'monty']
            for case in ['weighted', 'stateful']:
                for mode in ['cold', 'warm']:
                    for engine in engines:
                        cmd = shlex.join([f'{remote}/clear-speed', engine, case, mode, '10000', '201'])
                        identity['commands'].append(cmd)
                        rows = [json.loads(l) for l in run(ssh + [cmd]).splitlines()]
                        assert len(rows) == 201 and all(r['completed'] for r in rows)
                        for r in rows:
                            r['round'] = round_id
                            f.write(json.dumps(r, separators=(',', ':')) + '\n')
            print('clear round', round_id, 'completed', flush=True)
    with (out / 'clear-memory.jsonl').open('w') as f:
        for n, count in [(100, 32), (10000, 8)]:
            for engine in ['monty', 'monty-clear']:
                cmd = shlex.join([f'{remote}/clear-memory', engine, 'weighted', 'hold', str(n), str(count)])
                identity['commands'].append(cmd)
                rs = [json.loads(l) for l in run(ssh + [cmd]).splitlines()]
                assert len(rs) == 4
                for r in rs: f.write(json.dumps(r, separators=(',', ':')) + '\n')
    identity['postflight'] = run(ssh + ['uptime; free -m; ps'])
    (out / 'clear-identity.json').write_text(json.dumps(identity, indent=2) + '\n')
if __name__ == '__main__': main()
