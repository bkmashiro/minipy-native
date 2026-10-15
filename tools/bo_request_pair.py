#!/usr/bin/env python3
"""BO bounded old/new source-cold service pair; no build outside shared target."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

root = Path(__file__).resolve().parent.parent
scratch = Path.home() / ".hermes/cache/scratch"
base = scratch / "bo-base"
target = root.parent.parent / "target"
lock = scratch / "minipy-perf-build.lock"
out = root / "evidence/bo-json-print/request-cost.jsonl"
assert not out.exists(), "refuse to overwrite raw evidence"
assert not base.exists(), "remove stale baseline worktree manually before retry"
env = dict(os.environ, CARGO_TARGET_DIR=str(target), CARGO_BUILD_JOBS="1")
with lock.open("w") as handle:
    fcntl.flock(handle, fcntl.LOCK_EX)
    subprocess.run(["git", "worktree", "add", "--detach", str(base), "0b6c826"], cwd=root, check=True)
    try:
        shutil.copy2(root / "bench/src/bin/bo_request.rs", base / "bench/src/bin/bo_request.rs")
        manifest = base / "bench/Cargo.toml"
        with manifest.open("a") as f:
            f.write('\n[[bin]]\nname = "bo-request"\npath = "src/bin/bo_request.rs"\nrequired-features = ["string-probe"]\n')
        binaries = {}
        for arm, tree in (("B", base), ("A", root)):
            subprocess.run(["cargo", "+1.96.0", "clean", "--release", "--manifest-path", str(tree / "bench/Cargo.toml"), "-p", "minipy-native", "-p", "minipy-native-bench"], env=env, check=True)
            subprocess.run(["cargo", "+1.96.0", "build", "--offline", "--locked", "--release", "--features", "string-probe", "--manifest-path", str(tree / "bench/Cargo.toml"), "--bin", "bo-request"], env=env, check=True)
            saved = scratch / f"bo-{arm}-request"
            shutil.copy2(target / "release/bo-request", saved)
            binaries[arm] = (saved, hashlib.sha256(saved.read_bytes()).hexdigest())
        out.parent.mkdir(parents=True, exist_ok=True)
        with out.open("w") as f:
            for process, arm in enumerate("BAAB"):
                cases = ["scalar", "quote"] + (["string"] if arm == "A" else [])
                for case in cases:
                    for backend in ("tape", "native"):
                        exe, digest = binaries[arm]
                        run = subprocess.run([str(exe), backend, case, "40" if backend == "native" else "80"], capture_output=True, text=True, check=True, env=env)
                        rows = [json.loads(line) for line in run.stdout.splitlines()]
                        assert rows[0]["kind"] == "setup" and all(r["kind"] == "request" for r in rows[1:])
                        for index, row in enumerate(rows):
                            row.update(arm=arm, process=process, case=case, backend=backend, index=index, binary_sha256=digest, source_commit="0b6c826" if arm == "B" else "BO working tree")
                            f.write(json.dumps(row, separators=(",", ":")) + "\n")
                        f.flush()
                print("process", process, arm, flush=True)
        print("raw", out, hashlib.sha256(out.read_bytes()).hexdigest())
    finally:
        subprocess.run(["git", "worktree", "remove", "--force", str(base)], cwd=root, check=True)
