#!/usr/bin/env python3
"""Temporary diagnostic-wrapper ablation; restore production source in finally."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

root = Path(__file__).resolve().parent.parent
source = root / "src/frontend.rs"
target = root.parent.parent / "target"
lock = Path.home() / ".hermes/cache/scratch/minipy-perf-build.lock"
original = source.read_bytes()
old = [b"self.compile_stmt_inner(s, depth, in_loop, in_loop_body, out)\n            .map_err(|e| e.at(self.source, s.start().to_usize()))", b"self.expr_inner(e, depth, in_loop)\n            .map_err(|error| error.at(self.source, e.start().to_usize()))"]
new = [b"self.compile_stmt_inner(s, depth, in_loop, in_loop_body, out)", b"self.expr_inner(e, depth, in_loop)"]
for part in old:
    assert original.count(part) == 1
out = root / "evidence/bl-request-cost/ablation.jsonl"
assert not out.exists()
env = dict(os.environ, CARGO_TARGET_DIR=str(target), CARGO_BUILD_JOBS="1")
with lock.open("w") as handle:
    fcntl.flock(handle, fcntl.LOCK_EX)
    try:
        changed = original
        for a, b in zip(old, new):
            changed = changed.replace(a, b)
        source.write_bytes(changed)
        subprocess.run(["cargo", "+1.96.0", "clean", "--release", "--manifest-path", str(root / "bench/Cargo.toml"), "-p", "minipy-native", "-p", "minipy-native-bench"], check=True, env=env)
        subprocess.run(["cargo", "+1.96.0", "build", "--offline", "--locked", "--release", "--manifest-path", str(root / "bench/Cargo.toml"), "--bin", "interpreter_ab"], check=True, env=env)
        binary = target / "bl-N-interpreter_ab"
        shutil.copy2(target / "release/interpreter_ab", binary)
    finally:
        source.write_bytes(original)
    binaries = {"A": target / "bl-A-interpreter_ab", "N": binary}
    with out.open("w") as result:
        for process, arm in enumerate("ANNA"):
            for case, count in (("scalar", 0), ("weighted", 256)):
                run = subprocess.run([str(binaries[arm]), "interpreter", case, "service-cold", str(count), "100"], check=True, capture_output=True, text=True, env=env)
                output = [json.loads(line) for line in run.stdout.splitlines()]
                rows = [r for r in output if r["kind"] == "speed"]
                assert len(rows) == 100 and all(r["completed"] for r in rows)
                for row in rows:
                    row.update(arm=arm, process=process, binary_sha256=hashlib.sha256(binaries[arm].read_bytes()).hexdigest())
                    result.write(json.dumps(row, separators=(",", ":")) + "\n")
            result.flush()
            print(process, arm, flush=True)
