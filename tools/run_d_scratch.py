"""D's one bounded compiler-scratch experiment. Serial alternating runs, no builds.
Usage: python3 tools/run_d_scratch.py [ssh-key known-hosts host]
No arguments uses the two local benchmark binaries specified below.
All request observations, including each process's first compile, are retained.
"""
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'bench/results/d-script'
OUT.mkdir(exist_ok=True)
if len(sys.argv) == 4:
    key, known, host = sys.argv[1:]
    prefix = ['ssh', '-i', key, '-o', f'UserKnownHostsFile={known}', '-o', 'StrictHostKeyChecking=yes', '-o', 'BatchMode=yes', host]
    binaries = {'base': './script-bench-d-base', 'scratch': './script-bench-d-scratch'}
    location = 'linux'
else:
    assert len(sys.argv) == 1
    prefix = []
    binaries = {'base': '/Users/yuzhe/.hermes/cache/scratch/d-script-c11-m4', 'scratch': '/Users/yuzhe/.hermes/cache/scratch/d-script-scratch-m4'}
    location = 'm4'
path = OUT / f'scratch-{location}.jsonl'
assert not path.exists(), 'preserve previous experiment'
with path.open('w') as out:
    for round_id in range(3):
        order = ['base', 'scratch'] if round_id % 2 == 0 else ['scratch', 'base']
        for mode, rows, reps, delay in [('cold', 100, 60, 0), ('cold', 10_000, 60, 0), ('mixed', 100_000, 64, 1000)]:
            for arm in order:
                args = [binaries[arm], 'native', 'weighted', mode, str(rows), str(reps), str(delay)]
                if prefix:
                    import shlex
                    cmd = prefix + ['cd /root/minipy-native-c-v2 && MINIPY_FIXTURES=/root/minipy-native-c-v2/fixtures ' + shlex.join(args)]
                else:
                    cmd = args
                r = subprocess.run(cmd, cwd=ROOT, text=True, capture_output=True)
                if r.returncode:
                    raise RuntimeError(f'{cmd}: {r.stderr}')
                raw = [json.loads(line) for line in r.stdout.splitlines()]
                assert len(raw) == (1 if mode == 'mixed' else reps)
                for row in raw:
                    row.update(location=location, arm=arm, round=round_id)
                    out.write(json.dumps(row) + '\n')
                out.flush()
                print(location, round_id, arm, mode, rows, 'ok')
