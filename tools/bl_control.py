#!/usr/bin/env python3
"""Bounded service-cold paired control; run under the shared build lock."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

root = Path(__file__).resolve().parent.parent
baseline = root / "bl-baseline"
assert baseline.is_dir()
target = root.parent.parent / "target"
env = dict(os.environ, CARGO_TARGET_DIR=str(target), CARGO_BUILD_JOBS="1")
lock = Path.home() / ".hermes/cache/scratch/minipy-perf-build.lock"
out = root / "evidence/bl-request-cost/service-cold.jsonl"
out.parent.mkdir(parents=True, exist_ok=True)
assert not out.exists()
exe = target / "release/interpreter_ab"
binaries = {}
with lock.open("w") as handle, out.open("w") as result:
    fcntl.flock(handle, fcntl.LOCK_EX)
    for arm, tree in (("B", baseline), ("A", root)):
        saved = target / f"bl-{arm}-interpreter_ab"
        revision = subprocess.check_output(["git", "-C", str(tree), "rev-parse", "HEAD"], text=True).strip()
        if arm == "B":
            assert revision.startswith("89b898b")
        else:
            subprocess.run(["git", "-C", str(tree), "merge-base", "--is-ancestor", "e436812", "HEAD"], check=True)
        if arm == "B":
            shutil.copy2(root / "bench/src/bin/interpreter_ab.rs", tree / "bench/src/bin/interpreter_ab.rs")
        manifest = str(tree / "bench/Cargo.toml")
        subprocess.run(["cargo", "+1.96.0", "clean", "--release", "--manifest-path", manifest, "-p", "minipy-native", "-p", "minipy-native-bench"], check=True, env=env)
        subprocess.run(["cargo", "+1.96.0", "build", "--offline", "--locked", "--release", "--manifest-path", manifest, "--bin", "interpreter_ab"], check=True, env=env)
        shutil.copy2(exe, saved)
        binaries[arm] = saved
        print("built", arm, hashlib.sha256(saved.read_bytes()).hexdigest(), flush=True)
    for process, arm in enumerate("BAAB"):
        cases = [("scalar", 0), ("weighted", 256)]
        if arm == "A":
            cases += [("builtin-scalar", 0), ("builtin-quote", 256)]
        for case, n in cases:
            for engine in ("interpreter", "native"):
                run = subprocess.run([str(binaries[arm]), engine, case, "service-cold", str(n), "80"], check=True, capture_output=True, text=True, env=env)
                output = [json.loads(line) for line in run.stdout.splitlines()]
                assert len(output) == 81 and output[0]["kind"] == "setup" and not output[0]["prepared_user_code"]
                rows = output
                assert all(r["completed"] and r["kind"] == "speed" for r in rows[1:])
                for r in rows:
                    r.update(arm=arm, process=process, source_commit="89b898b" if arm == "B" else "e436812", harness_overlay=True, binary_sha256=hashlib.sha256(binaries[arm].read_bytes()).hexdigest())
                    result.write(json.dumps(r, separators=(",", ":")) + "\n")
                result.flush()
        print("process", process, arm, flush=True)
