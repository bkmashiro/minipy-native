"""N actual guest execution of already-built tests and separate diagnostic counts."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'bench/results/n-compact'

def run(args): return subprocess.check_output(args,text=True,stderr=subprocess.STDOUT)
def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()

def main():
    host,key,known=sys.argv[1:]
    opts=['-i',key,'-o',f'UserKnownHostsFile={known}','-o','StrictHostKeyChecking=yes','-o','BatchMode=yes']
    ssh=['ssh',*opts,host]
    remote='/root/minipy-native-n'
    ident={'commands':[],'executables':{},'counts':{},'test_count':0}
    logs=[]
    artifacts=[json.loads(line) for line in (ROOT/'target/n-compact/linux-tests-build.json').read_text().splitlines()]
    executables={r['target']['name']:Path(r['executable']) for r in artifacts if r.get('reason')=='compiler-artifact' and r.get('executable') and r['profile']['test']}
    assert set(executables)=={'minipy_native','minipy','acceptance','boundaries','fixture_lifecycle','fixture_reference','interpreter','scripts'}
    def upload(name,p):
        run(['scp',*opts,str(p),f'{host}:{remote}/{name}'])
        assert run(ssh+[f'sha256sum {remote}/{name}']).split()[0]==sha(p)
        ident['executables'][name]={'sha256':sha(p),'bytes':p.stat().st_size}
    for name,p in sorted(executables.items()):
        upload('test-'+name,p)
        cmd=f'{remote}/test-{name} --test-threads=1'
        ident['commands'].append(cmd)
        output=run(ssh+[cmd])
        assert 'test result: ok.' in output and '0 failed' in output
        import re
        match=re.search(r'test result: ok\. (\d+) passed',output)
        assert match is not None
        ident['test_count']+=int(match[1])
        logs.append(cmd+'\n'+output)
    assert ident['test_count']==66
    for build in ['baseline','tape']:
        p=ROOT/'target/n-compact'/f'{build}-counts-linux'
        upload(build+'-counts',p)
        for case in ['weighted','stateful']:
            cmd=f'{remote}/{build}-counts {case} 256'
            ident['commands'].append(cmd)
            output=run(ssh+[cmd])
            ident['counts'][f'{build}/{case}']=output
            logs.append(cmd+'\n'+output)
    (OUT/'linux-tests.log').write_text('\n'.join(logs))
    (OUT/'linux-verification.json').write_text(json.dumps(ident,indent=2)+'\n')
    print('actual Linux tests',ident['test_count'])
    for k,v in ident['counts'].items(): print(k,v.strip())

if __name__=='__main__': main()
