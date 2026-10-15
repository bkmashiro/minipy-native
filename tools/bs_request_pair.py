#!/usr/bin/env python3
"""Bounded BQ/BS source-cold BAAB pair with a shared bench harness."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

root = Path(__file__).resolve().parent.parent
scratch = Path.home() / ".hermes/cache/scratch"
base = scratch / "bs-bq-base"
target = root.parent.parent / "target"
out = root / "evidence/bs-string-integration/request-cost.jsonl"
assert not out.exists(), "refuse to overwrite raw evidence"
assert not base.exists(), "stale baseline worktree; inspect before retry"
env = dict(os.environ, CARGO_TARGET_DIR=str(target), CARGO_BUILD_JOBS="1")
source = (root / "bench/src/bin/bo_request.rs").read_bytes()
with (scratch / "minipy-perf-build.lock").open("a+") as handle:
    fcntl.flock(handle, fcntl.LOCK_EX)
    subprocess.run(["git", "worktree", "add", "--detach", str(base), "6b48ebc"], cwd=root, check=True)
    try:
        (base / "bench/src/bin/bo_request.rs").write_bytes(source)
        binaries = {}
        for arm, tree in (("B", base), ("A", root)):
            subprocess.run(["cargo", "+1.96.0", "build", "--offline", "--locked", "--release", "--features", "string-probe", "--manifest-path", str(tree / "bench/Cargo.toml"), "--bin", "bo-request"], env=env, check=True)
            saved = scratch / f"bs-{arm}-request"
            shutil.copy2(target / "release/bo-request", saved)
            binaries[arm] = (saved, hashlib.sha256(saved.read_bytes()).hexdigest())
        out.parent.mkdir(parents=True, exist_ok=True)
        with out.open("w") as f:
            for process, arm in enumerate("BAAB"):
                for case in ["scalar", "quote", "string", "plain"] + (["joined"] if arm == "A" else []):
                    for backend in ("tape", "native"):
                        exe, digest = binaries[arm]
                        run = subprocess.run([str(exe), backend, case, "24"], capture_output=True, text=True, check=True, env=env)
                        rows = [json.loads(line) for line in run.stdout.splitlines()]
                        assert rows[0]["kind"] == "setup" and len(rows) == 26
                        for index, row in enumerate(rows):
                            row.update(arm=arm, process=process, case=case, backend=backend, index=index, binary_sha256=digest, harness_sha256=hashlib.sha256(source).hexdigest(), source_commit="6b48ebc" if arm == "B" else "BS working tree")
                            f.write(json.dumps(row, separators=(",", ":")) + "\n")
                        f.flush()
                print("process", process, arm, flush=True)
        print("raw", out, hashlib.sha256(out.read_bytes()).hexdigest())
    finally:
        subprocess.run(["git", "worktree", "remove", "--force", str(base)], cwd=root, check=True)
