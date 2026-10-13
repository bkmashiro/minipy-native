from pathlib import Path
import subprocess, shutil, csv, statistics, json
out=Path(__file__).parent
for name in ['diagnostics','owner']:
 d=out/name
 shutil.copy2(out/'owner_probe.rs',d/'src/bin/owner_probe.rs')
 subprocess.run(['cargo','+1.96.0','clean','--release','-p','minipy-native','--manifest-path',str(d/'Cargo.toml'),'--target-dir',str(out/'target')],check=True)
 subprocess.run(['cargo','+1.96.0','build','--release','--locked','--bin','owner_probe','--manifest-path',str(d/'Cargo.toml'),'--target-dir',str(out/'target')],check=True)
 shutil.copy2(out/'target/release/owner_probe',out/f'probe-{name}')
raw=out/'raw'; raw.mkdir(exist_ok=True)
commands=[]
for round in range(4):
 order=['baseline','diagnostics'] if round%2==0 else ['diagnostics','baseline']
 for name in order:
  cmd=[str(out/f'generic-{name}'),'generic'];commands.append(cmd)
  with (raw/f'generic-{round}-{name}.csv').open('w') as f:
   subprocess.run(cmd,stdout=f,check=True)
 order=['diagnostics','owner'] if round%2==0 else ['owner','diagnostics']
 for name in order:
  cmd=[str(out/f'probe-{name}')];commands.append(cmd)
  with (raw/f'owner-{round}-{name}.csv').open('w') as f:
   subprocess.run(cmd,stdout=f,check=True)
summary={}
for family,names,columns in [('generic',['baseline','diagnostics'],['compile_ns','conversion_ns','execute_cleanup_ns','total_ns','code_bytes']),('owner',['diagnostics','owner'],['generation_ns','freeze_ns','execution_cleanup_ns','total_ns','payload_bytes'])]:
 for name in names:
  groups={}
  for round in range(4):
   with (raw/f'{family}-{round}-{name}.csv').open() as f:
    rows=list(csv.DictReader(line for line in f if not line.startswith('#')))
   if family=='generic': assert len(rows)==480
   else: assert len(rows)==75
   for r in rows:
    if int(r['sample'])<5: continue
    if family=='generic' and r['engine']!='native': continue
    key=','.join(r.get(c,'') for c in ['rows','mode','chunk'])
    groups.setdefault(key,[]).append(r)
  summary[family+'-'+name]={k:{'count':len(rs),**{c:statistics.median(int(r[c]) for r in rs) for c in columns}} for k,rs in groups.items()}
(out/'commands.json').write_text(json.dumps(commands,indent=2)+'\n')
(out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary,indent=2))
