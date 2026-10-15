#!/usr/bin/env python3
"""Bounded old-source B/A/A/B control twice; no private inputs."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parent.parent
baseline = root / "baseline"
out = root / "evidence/bk-integration/source-cold.jsonl"
out.parent.mkdir(parents=True, exist_ok=True)
assert baseline.is_dir() and not out.exists()
target = root.parent.parent / "target"
env = dict(os.environ, CARGO_TARGET_DIR=str(target), CARGO_BUILD_JOBS="1")
exe = target / "release/interpreter_ab"
lock = Path.home() / ".hermes/cache/scratch/minipy-perf-build.lock"
with lock.open("w") as handle, out.open("w") as result:
    fcntl.flock(handle, fcntl.LOCK_EX)
    for index, arm in enumerate("BAABBAAB"):
        tree = baseline if arm == "B" else root
        revision = subprocess.check_output(["git", "-C", str(tree), "rev-parse", "HEAD"], text=True).strip()
        dirty = bool(subprocess.check_output(["git", "-C", str(tree), "status", "--porcelain", "--untracked-files=no"], text=True))
        manifest = str(tree / "bench/Cargo.toml")
        subprocess.run(["cargo", "+1.96.0", "clean", "--release", "--manifest-path", manifest, "-p", "minipy-native", "-p", "minipy-native-bench"], check=True, env=env)
        subprocess.run(["cargo", "+1.96.0", "build", "--offline", "--locked", "--release", "--manifest-path", manifest, "--bin", "interpreter_ab"], check=True, env=env)
        binary_sha = hashlib.sha256(exe.read_bytes()).hexdigest()
        rows = []
        for case, count in (("scalar", 0), ("weighted", 256)):
            for engine in ("interpreter", "native"):
                proc = subprocess.run([str(exe), engine, case, "cold", str(count), "50"], check=True, capture_output=True, text=True, env=env)
                for line in proc.stdout.splitlines():
                    row = json.loads(line)
                    assert row["completed"] and row["kind"] == "speed"
                    row.update(arm=arm, process=index, source_commit=revision, source_dirty=dirty, binary_sha256=binary_sha)
                    rows.append(row)
        assert len(rows) == 200
        for row in rows:
            result.write(json.dumps(row, separators=(",", ":")) + "\n")
        result.flush()
        print(index, arm, revision[:12], binary_sha[:12], len(rows), flush=True)
