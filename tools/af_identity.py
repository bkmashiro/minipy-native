"""Capture small AF source/runtime identities, without build/raw copies."""
import hashlib,json,subprocess,sys
from pathlib import Path
base=Path(sys.argv[1]);out=base/'af-capacity'
files=[out/'worker',out/'interpreter_ab',out/'af_capacity.py',out/'source/bench/src/af_ipc.rs',out/'source/bench/src/bin/interpreter_ab.rs',out/'source/bench/Cargo.toml',out/'source/bench/Cargo.lock',base/'ad-corrected/native.py',base/'w-guest/runtime/dist/pysolate.wasm',base/'w-guest/runtime/runner.go',base/'w-guest/runtime/guest/runtime.c']
files+=sorted((out/'source/src').rglob('*.rs'))+sorted((out/'source/fixtures').glob('*.py'))+sorted((out/'harness').glob('*'))
hashes={str(p.relative_to(base)):{'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'bytes':p.stat().st_size} for p in files if p.is_file()}
identity=out/'identity.json'
if identity.exists():
    assert json.loads(identity.read_text())['files']==hashes,'measured source changed'
    print('postflight immutable hashes match',len(hashes))
else:
    identity.write_text(json.dumps({'cohort':'af-open-loop-v1','files':hashes,'runtime_frozen_commit':'969e679','ad_evidence_commit':'c3dff6a74dcaa3136b2cb8b48635cd70199c253b','environment':subprocess.check_output(['bash','-lc','uname -a; python3 --version; go version; cargo +1.96.0 --version; lscpu; free -m; ps -eo pid,comm,pcpu --sort=-pcpu'],text=True),'affinity':{'service_tree':[2],'driver':[4],'unused_smt_siblings':[3,5]},'ad_cpu':'Host getrusage SELF + native parent CPUCLOCK_SCHED + monotonic max cumulative reaped-child RUSAGE_CHILDREN; no sampling RPC','locks':[str(base/'compute.lock')]},indent=2)+'\n')
    print('identity captured',len(hashes))
