"""N bounded paired experiment. Commands:
compact_ab.py attribution
compact_ab.py linux HOST KEY KNOWN_HOSTS
Uses existing interpreter_ab; source-once and explicit reuse are separate.
"""
import gzip
import hashlib
import json
from contextlib import nullcontext
from pathlib import Path
import shlex
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'bench/results/n-compact'
SHAPES = [('scalar', 0), ('weighted', 100), ('stateful', 100),
          ('weighted', 10000), ('stateful', 100000)]
ORDER = ['AB', 'BA', 'BA', 'AB']

def run(args, **kw):
    return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT, **kw)

def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()

def identity():
    return {'head': run(['git', 'rev-parse', 'HEAD']).strip(),
            'patch': run(['git', 'diff', '--binary', '--', 'src', 'tests']),
            'order': ORDER, 'commands': [], 'binaries': {},
            'source_sha256': {str(p.relative_to(ROOT)): sha(p)
                              for pattern in ['src/**/*.rs', 'fixtures/*.py', 'tests/*.rs',
                                              'bench/src/bin/interpreter_ab.rs', 'bench/src/v2_common.rs',
                                              'Cargo*', 'bench/Cargo*', 'tools/compact*.py']
                              for p in ROOT.glob(pattern) if p.is_file()},
            'rust': run(['rustc', '+1.96.0', '--version']).strip()}

def attribution():
    OUT.mkdir(parents=True, exist_ok=True)
    raw = OUT / 'frontend-macos.jsonl.gz'
    assert not raw.exists(), 'preserve raw'
    ident = identity()
    ident['platform'] = run(['uname', '-a']).strip()
    ident['stages'] = {'baseline': '8395039', 'bitset': 'a4d68c1', 'frontend': '81d5df0'}
    for name in ident['stages']:
        p = ROOT / 'target/n-compact' / name
        ident['binaries'][name] = {'sha256': sha(p), 'bytes': p.stat().st_size}
    count = 0
    with gzip.open(raw, 'wt') as f:
        for pair, order in enumerate(ORDER):
            for comparison, names in [('bitset', ['baseline', 'bitset']), ('container', ['bitset', 'frontend'])]:
                for engine in ['interpreter', 'native']:
                    for case, n in SHAPES[:3]:
                        for name in (names if order == 'AB' else names[::-1]):
                            cmd = [str(ROOT / 'target/n-compact' / name), engine, case, 'cold', str(n), '121']
                            ident['commands'].append(cmd)
                            rows = [json.loads(l) for l in run(cmd).splitlines()]
                            assert len(rows) == 121 and all(r['completed'] for r in rows)
                            for r in rows:
                                r.update(build=name, comparison=comparison, round=pair)
                                f.write(json.dumps(r, separators=(',', ':')) + '\n')
                            count += len(rows)
                            f.flush()
    assert count == 4 * 2 * 2 * 3 * 2 * 121
    ident['raw_count'] = count
    (OUT / 'frontend-macos-identity.json').write_text(json.dumps(ident, indent=2)+'\n')
    print('attribution raw', count)

def linux(host, key, known, resume=False):
    opts = ['-i', key, '-o', f'UserKnownHostsFile={known}',
            '-o', 'StrictHostKeyChecking=yes', '-o', 'BatchMode=yes']
    ssh = ['ssh', *opts, host]
    remote = '/root/minipy-native-n'
    OUT.mkdir(parents=True, exist_ok=True)
    raw = OUT / 'linux.jsonl.gz'
    memory = OUT / 'memory.jsonl'
    if resume:
        assert raw.exists() and memory.exists() and memory.stat().st_size == 0
        with gzip.open(raw, 'rt') as f:
            retained = [json.loads(line) for line in f]
        assert len(retained) == 20560 and all(r['completed'] for r in retained)
    else:
        assert not raw.exists() and not memory.exists(), 'preserve raw'
    ident = identity()
    if resume:
        ident['failure'] = 'Speed completed; memory assertion caught mixed-hold reps=40 (40 windows). No speed rerun. Memory resumed with reps=1, unchanged binaries. Speed preflight was not checkpointed by original runner; this preflight is memory-resume only.'
    ident.update(baseline_commit='839503936b4d1eeca892ebeeed49abcb378be4bf',
                 preflight=run(ssh+['uname -a; uptime; free -m; df -h /; ps']),
                 zig=run(['zig', 'version']).strip(),
                 accounting='K/H five shapes and weights; mixed 9 windows keep>=2; '
                            'standalone 41 keep>=11; chunk256; ready/delay1000; '
                            'cold includes parse/prepare/provider/execute/unit reclaim; warm explicit reuse',
                 vm_initial_state='off; existing 2 CPU/2048 MiB VM started exclusively')
    run(ssh+[f'mkdir -p {remote}'])
    for name, local in [('base','baseline-linux'), ('candidate','tape-linux'),
                        ('base-memory','baseline-memory-linux'), ('candidate-memory','tape-memory-linux')]:
        p = ROOT / 'target/n-compact' / local
        run(['scp', *opts, str(p), f'{host}:{remote}/{name}'])
        assert run(ssh+[f'sha256sum {remote}/{name}']).split()[0] == sha(p)
        ident['binaries'][name] = {'sha256': sha(p), 'bytes': p.stat().st_size}
    count = 0
    with (nullcontext(None) if resume else gzip.open(raw, 'wt')) as f:
        def collect(build, case, mode, n, reps, pair, delay):
            nonlocal count
            cmd = f'MINIPY_CHUNK=256 MINIPY_TOOL_DELAY_US={delay} ' + shlex.join(
                [f'{remote}/{build}', 'interpreter', case, mode, str(n), str(reps)])
            ident['commands'].append(cmd)
            if resume:
                count += reps * (40 if mode.startswith('mixed-') else 1)
                return
            rows = [json.loads(l) for l in run(ssh+[cmd]).splitlines()]
            assert len(rows) == reps * (40 if mode.startswith('mixed-') else 1)
            assert all(r['completed'] for r in rows)
            assert f is not None
            for r in rows:
                r.update(build=build, round=pair, chunk=256)
                f.write(json.dumps(r, separators=(',', ':'))+'\n')
            count += len(rows)
            f.flush()
        for pair, order in enumerate(ORDER):
            names = ['base','candidate'] if order == 'AB' else ['candidate','base']
            for mix in ['light','scan']:
                for mode, delay in [('mixed-cold',0),('mixed-cold',1000),('mixed-warm',0)]:
                    for build in names:
                        collect(build,mix,mode,0,9,pair,delay)
            for case,n in SHAPES:
                for mode in ['cold','warm']:
                    for build in names:
                        collect(build,case,mode,n,41,pair,0)
            print('Linux pair',pair,'complete',flush=True)
    assert count == 20560
    mem_count=0
    with memory.open('w') as f:
        for pair in range(2):
            for build in (['base','candidate'] if pair==0 else ['candidate','base']):
                cells=[(mix,'mixed-hold',0,1,'unique') for mix in ['light','scan']]
                cells += [(case,'hold',n,32,code) for case,n in [('weighted',100),('stateful',100000)] for code in ['unique','shared']]
                for case,mode,n,reps,code in cells:
                    cmd=f'MINIPY_HOLD_CODE={code} ' + shlex.join([f'{remote}/{build}-memory','interpreter',case,mode,str(n),str(reps)])
                    ident['commands'].append(cmd)
                    rows=[json.loads(l) for l in run(ssh+[cmd]).splitlines()]
                    assert len(rows)==4
                    for r in rows:
                        r.update(build=build,round=pair,code=code)
                        f.write(json.dumps(r,separators=(',',':'))+'\n')
                    mem_count+=len(rows)
                    f.flush()
    assert mem_count==96
    ident.update(raw_count=count,memory_count=mem_count,postflight=run(ssh+['uptime; free -m; df -h /; ps']))
    (OUT/'linux-identity.json').write_text(json.dumps(ident,indent=2)+'\n')
    print('Linux raw',count,'memory',mem_count)

if __name__=='__main__':
    if sys.argv[1]=='attribution': attribution()
    else:
        assert sys.argv[1] in ['linux','linux-memory-resume']
        linux(*sys.argv[2:], resume=sys.argv[1]=='linux-memory-resume')
