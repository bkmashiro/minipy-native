"""Single assembly-reference refinement, original addon raw is never replaced.
Usage: final_assembly_refinement.py HOST KEY KNOWN SPEED MEMORY TEST
"""
import gzip
import hashlib
import json
from pathlib import Path
import shlex
import subprocess
import sys
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'bench/results/l-final';REMOTE='/root/minipy-native-l'


def main():
    host,key,known,speed,memory,test,*extra=sys.argv[1:]
    assert extra in [[],['full-only']]
    full_only=bool(extra)
    prefix='assembly-full' if full_only else 'assembly-v2'
    opts=['-i',key,'-o',f'UserKnownHostsFile={known}','-o','StrictHostKeyChecking=yes','-o','BatchMode=yes']
    ssh=['ssh',*opts,host]
    sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
    def run(argv):return subprocess.check_output(argv,text=True,stderr=subprocess.STDOUT,timeout=240)
    identity={'commands':[],'binaries':{},'source_sha256':{p:sha(ROOT/p) for p in ['bench/src/weighted_aarch64.S','bench/src/assembly_reference.rs','bench/src/bin/assembly_ab.rs','tools/final_assembly_refinement.py']},'preflight':run(ssh+['uptime; free -m; ps']), 'failures':[]}
    def command(cmd):
        identity['commands'].append(cmd)
        try:return run(ssh+[cmd])
        except Exception as exc:identity['failures'].append(str(exc));raise
    try:
        for name,p in [(prefix,speed),(prefix+'-memory',memory),(prefix+'-test',test)]:
            run(['scp',*opts,p,f'{host}:{REMOTE}/{name}'])
            assert run(ssh+[f'sha256sum {REMOTE}/{name}']).split()[0]==sha(p)
            identity['binaries'][name]={'sha256':sha(p),'bytes':Path(p).stat().st_size}
        (OUT/(prefix+'-tests.log')).write_text(command(f'MINIPY_FIXTURES={REMOTE}/fixtures {REMOTE}/{prefix}-test --test-threads=1'))
        raw=OUT/(prefix+'.jsonl.gz');assert not raw.exists(),'preserve raw'
        count=0
        with gzip.open(raw,'wt') as f:
            for ri in range(3):
                order=['assembly','rust'] if ri%2==0 else ['rust','assembly']
                for case in ['weighted','stateful']:
                    for mode in (['full'] if full_only else ['kernel','full']):
                        for n in ([0,100,10000,100000] if mode=='kernel' else [100,10000,100000]):
                            for delay in ([0] if mode=='kernel' else [0,1000]):
                                for engine in order:
                                    rs=[json.loads(l) for l in command(shlex.join([f'{REMOTE}/{prefix}',engine,mode,case,str(n),'101',str(delay)])).splitlines()]
                                    assert len(rs)==101 and all(r['completed'] for r in rs)
                                    for r in rs:r.update(round=ri,delay_us=delay,assembly_revision=2);f.write(json.dumps(r)+'\n')
                                    count+=len(rs);f.flush()
                print('assembly refinement round',ri,'complete',count,flush=True)
        expected=7272 if full_only else 12120
        assert count==expected
        rows=[]
        for ri in range(3):
            for tasks in [1,8,32]:
                rs=[json.loads(l) for l in command(f'{REMOTE}/{prefix}-memory assembly hold weighted 10000 {tasks} 0').splitlines()]
                assert [r['phase'] for r in rs]==['startup','load_wait','record_wait','reclaimed']
                for r in rs:r.update(round=ri,assembly_revision=2);rows.append(r)
        assert len(rows)==36
        (OUT/(prefix+'-memory.jsonl')).write_text(''.join(json.dumps(r)+'\n' for r in rows))
        with gzip.open(raw,'rt') as f:rs=[json.loads(l) for l in f]
        assert len(rs)==expected and all(r['completed'] for r in rs)
        identity['verified_counts']={'speed':expected,'memory':36}
    finally:
        identity['postflight']=run(ssh+['uptime; free -m; ps'])
        (OUT/(prefix+'-identity.json')).write_text(json.dumps(identity,indent=2)+'\n')


if __name__=='__main__':main()
