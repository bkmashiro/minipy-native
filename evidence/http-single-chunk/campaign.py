#!/usr/bin/env python3
"""Run the locked, same-target release BAAB loopback campaign."""
from pathlib import Path
import fcntl, hashlib, os, subprocess, sys, time

repo = Path(__file__).resolve().parents[2]
src = repo / "src/http_provider.rs"
evidence = Path(sys.argv[1]).resolve() if len(sys.argv) == 2 else Path(__file__).resolve().parent / "release-baab"
if evidence.exists():
    raise SystemExit("use a new evidence directory; existing measurements are never overwritten")
evidence.mkdir(parents=True)
lock_path = Path("/Users/yuzhe/.hermes/cache/scratch/minipy-perf-build.lock")
text = src.read_text()
candidate = '''            let mut first_chunk = None;\n            let mut chunks: Option<Vec<Bytes>> = None;\n            let mut total = 0usize;\n            let mut chunk_count = 0usize;'''
baseline = '''            let mut chunks = Vec::new();\n            let mut total = 0usize;'''
if candidate not in text and baseline not in text:
    raise SystemExit("collector not found exactly; refusing source edit")
old_guard = 'if data.len() > self.body_bytes.saturating_sub(total) || chunk_count >= 4096 {'
new_guard = 'if data.len() > self.body_bytes.saturating_sub(total) || chunks.len() >= 4096 {'
old_push = '''                    chunk_count += 1;\n                    if let Some(chunks) = &mut chunks {\n                        chunks.push(data);\n                    } else if let Some(first) = first_chunk.take() {\n                        chunks = Some(vec![first, data]);\n                    } else {\n                        first_chunk = Some(data);\n                    }'''
new_push = '                    chunks.push(data);'
old_end = '''            let bytes = if let Some(chunks) = chunks {\n                let mut merged = BytesMut::with_capacity(total);\n                for data in chunks {\n                    merged.extend_from_slice(&data);\n                }\n                self.copied_bytes.fetch_add(total, Ordering::Relaxed);\n                merged.freeze()\n            } else {\n                first_chunk.unwrap_or_default()\n            };'''
new_end = '''            let bytes = if chunks.len() == 1 {\n                chunks.pop().unwrap()\n            } else {\n                let mut merged = BytesMut::with_capacity(total);\n                for data in chunks {\n                    merged.extend_from_slice(&data);\n                }\n                self.copied_bytes.fetch_add(total, Ordering::Relaxed);\n                merged.freeze()\n            };'''

def switch(variant):
    current = src.read_text()
    if (variant == "baseline" and baseline in current) or (variant == "candidate" and candidate in current):
        return
    if variant == "baseline":
        if candidate not in current: raise SystemExit("candidate source block missing")
        current = current.replace(candidate, baseline, 1).replace(old_guard, new_guard, 1).replace(old_push, new_push, 1).replace(old_end, new_end, 1)
    else:
        if baseline not in current: raise SystemExit("baseline source block missing")
        current = current.replace(baseline, candidate, 1).replace(new_guard, old_guard, 1).replace(new_push, old_push, 1).replace(new_end, old_end, 1)
    src.write_text(current)

env = os.environ.copy()
env.update(CARGO_TARGET_DIR=str(repo / "target"), CARGO_BUILD_JOBS="1")
lock_path.parent.mkdir(parents=True, exist_ok=True)
with lock_path.open("a") as lock:
    fcntl.flock(lock, fcntl.LOCK_EX)
    meta = subprocess.run(["rustc", "+1.96.0", "-Vv"], capture_output=True, text=True, check=True).stdout
    (evidence / "build-identity.txt").write_text(f"date_epoch={int(time.time())}\nrustc -Vv:\n{meta}target_dir={env['CARGO_TARGET_DIR']}\nCARGO_BUILD_JOBS=1\ncommand=cargo +1.96.0 --offline --locked test --features tool-json --release\n")
    original = src.read_text()
    try:
        for index, variant in enumerate(("baseline", "candidate", "candidate", "baseline"), 1):
            switch(variant)
            h = hashlib.sha256(src.read_bytes()).hexdigest()
            path = evidence / f"run-{index}.txt"
            with path.open("w") as out:
                out.write(f"variant={variant} source_sha256={h}\n")
                for args, extra in [
                    (["cargo", "+1.96.0", "--offline", "--locked", "test", "--features", "tool-json", "--release", "--test", "http_collect_cost", "--", "--ignored", "--nocapture"], {"HTTP_VARIANT":variant}),
                    (["cargo", "+1.96.0", "--offline", "--locked", "test", "--features", "tool-json", "--release", "--lib", "http_collect_cost_tests", "--", "--nocapture"], {}),
                ]:
                    runenv = env.copy(); runenv.update(extra)
                    p = subprocess.run(args, cwd=repo, env=runenv, stdout=out, stderr=subprocess.STDOUT)
                    out.flush()
                    if p.returncode: raise SystemExit(f"{variant} run {index} failed; see {path}")
            print(f"completed {index}/4 {variant} source_sha256={h}", flush=True)
    finally:
        src.write_text(original)
print("campaign done")
