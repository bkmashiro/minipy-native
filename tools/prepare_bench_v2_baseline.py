"""Overlay the identical integrated bench on fixed runtime 564381d, in ignored target.
No worktree/branch or source modification. Build it with a separate target dir.
Usage: python3 tools/prepare_bench_v2_baseline.py [integrated-commit]
"""
import io
from pathlib import Path
import subprocess
import sys
import tarfile

ROOT = Path(__file__).resolve().parents[1]
BASELINE = '564381d304be69d1fd870d4e92530c8024b7c394'
commit = sys.argv[1] if len(sys.argv) > 1 else 'HEAD'
commit = subprocess.check_output(['git', 'rev-parse', commit], cwd=ROOT, text=True).strip()
output = ROOT / 'bench/target/c-baseline-source'
if output.exists():
    raise SystemExit('baseline source already exists; use the recorded snapshot or remove only this ignored directory')
output.mkdir(parents=True)
for revision, paths in [(BASELINE, ['Cargo.toml', 'Cargo.lock', 'src', 'fixtures']),
                        (commit, ['bench/Cargo.toml', 'bench/Cargo.lock', 'bench/src'])]:
    archive = subprocess.check_output(['git', 'archive', revision, '--', *paths], cwd=ROOT)
    with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
        # Only our trusted git archive of the enumerated source paths, Python 3.9 compatible.
        tar.extractall(output)
print(f'baseline_runtime={BASELINE}\nidentical_harness={commit}\nsource={output}')
