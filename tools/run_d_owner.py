"""Serial, isolated D owner experiment. Never applies the owner API patch.
Usage: python3 tools/run_d_owner.py ssh-key known-hosts host
Deploy baseline c0afa8c and isolated c0afa8c + B owner-candidate.patch first.
"""
import json
from pathlib import Path
import shlex
import subprocess
import sys
key, known, host = sys.argv[1:]
ssh = ['ssh', '-i', key, '-o', f'UserKnownHostsFile={known}', '-o', 'StrictHostKeyChecking=yes', '-o', 'BatchMode=yes', host]
root = Path(__file__).resolve().parents[1]
out = root / 'bench/results/d-script/owner-isolated-linux.jsonl'
assert not out.exists()
with out.open('w') as f:
    def run(binary, engine, mode, reps, arm, round_id):
        args = [binary, engine, 'weighted', mode, '1000000', str(reps), '0']
        cmd = ssh + ['cd /root/minipy-native-c-v2 && MINIPY_FIXTURES=/root/minipy-native-c-v2/fixtures ' + shlex.join(args)]
        r = subprocess.run(cmd, text=True, capture_output=True, check=True)
        raw = [json.loads(x) for x in r.stdout.splitlines()]
        assert len(raw) == (4 if mode == 'hold' else reps)
        for row in raw:
            row.update(arm=arm, round=round_id, isolated_unapproved_api=True)
            f.write(json.dumps(row) + '\n')
        f.flush()
        print(arm, mode, round_id, 'ok')
    for round_id in range(3):
        arms = ['base','owner'] if round_id % 2 == 0 else ['owner','base']
        for mode in ['cold','warm']:
            for arm in arms:
                binary = './d-print-growth-speed-linux' if arm == 'base' else './d-owner-speed-linux'
                run(binary, 'native', mode, 20, arm, round_id)
    for arm in ['base','owner']:
        binary = './d-print-growth-alloc-linux' if arm == 'base' else './d-owner-memory-linux'
        run(binary, 'native', 'hold', 1, arm, 0)
