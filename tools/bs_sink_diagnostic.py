#!/usr/bin/env python3
"""One bounded BS sink allocation/wait diagnostic; never overwrite raw evidence."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parent.parent
scratch = Path.home() / ".hermes/cache/scratch"
out = root / "evidence/bs-string-integration/sink-diagnostic.jsonl"
assert not out.exists(), "refuse to overwrite raw evidence"
env = dict(os.environ, CARGO_TARGET_DIR=str(root.parent.parent / "target"), CARGO_BUILD_JOBS="1")
with (scratch / "minipy-perf-build.lock").open("a+") as lock:
    fcntl.flock(lock, fcntl.LOCK_EX)
    subprocess.run(["cargo", "+1.96.0", "build", "--offline", "--locked", "--release", "--features", "string-probe", "--manifest-path", str(root / "bench/Cargo.toml"), "--bin", "bs-diag"], check=True, env=env)
    binary = root.parent.parent / "target/release/bs-diag"
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    out.parent.mkdir(parents=True, exist_ok=True)
    with out.open("w") as f:
        for backend in ("tape", "native"):
            for length in ("short", "long"):
                for gate in ("ready", "gated"):
                    run = subprocess.run([str(binary), backend, length, gate], capture_output=True, text=True, check=True, env=env)
                    rows = [json.loads(line) for line in run.stdout.splitlines()]
                    assert len(rows) == 26
                    for row in rows:
                        row["binary_sha256"] = digest
                        f.write(json.dumps(row, separators=(",", ":")) + "\n")
                    f.flush()
                    print(backend, length, gate, flush=True)
    print(out, "sha256", hashlib.sha256(out.read_bytes()).hexdigest())
