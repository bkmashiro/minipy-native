"""Small final D script comparison, with ordinary Monty executing all user logic.
Usage: python3 tools/run_d_final.py ssh-key known-hosts host
No build, source cache, host business helper, or concurrent measurement processes.
"""
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
key,known,host=sys.argv[1:]
ssh=['ssh','-i',key,'-o',f'UserKnownHostsFile={known}','-o','StrictHostKeyChecking=yes','-o','BatchMode=yes',host]
root=Path(__file__).resolve().parents[1]
path=root/'bench/results/d-script'/os.environ.get('D_OUT','final-linux.jsonl')
binary=os.environ.get('D_BIN','./d-final-script-speed-linux')
assert not path.exists()
with path.open('w') as f:
    for round_id in range(3):
        engines=['native','monty'] if round_id%2==0 else ['monty','native']
        for case,n in [('scalar',0),('weighted',100),('weighted',10000),('legacy',100),('legacy',10000)]:
            for mode in ['cold','warm']:
                for engine in (engines if case!='legacy' else ['native']):
                    args=[binary,engine,case,mode,str(n),'40','0']
                    command=ssh+['cd /root/minipy-native-c-v2 && MINIPY_FIXTURES=/root/minipy-native-c-v2/fixtures '+shlex.join(args)]
                    r=subprocess.run(command,text=True,capture_output=True,check=True)
                    rows=[json.loads(x) for x in r.stdout.splitlines()]
                    assert len(rows)==40 and all(x['completed'] for x in rows)
                    for row in rows:
                        row['round']=round_id
                        f.write(json.dumps(row)+'\n')
                    f.flush()
                    print(round_id,engine,case,n,mode,'ok')
