"""BI paired source-cold control; caller holds minipy-perf-build.lock."""
import json
import hashlib
import pathlib
import subprocess
import sys

arm, output = sys.argv[1:]
assert arm in {"base", "candidate"}
path = pathlib.Path(output)
path.parent.mkdir(parents=True, exist_ok=True)
exe = pathlib.Path("/Users/yuzhe/projects/minipy-native/target/release/interpreter_ab")
binary_sha256 = hashlib.sha256(exe.read_bytes()).hexdigest()
source_tree = pathlib.Path("baseline" if arm == "base" else ".")
source_sha = subprocess.check_output(["git", "-C", str(source_tree), "rev-parse", "HEAD"], text=True).strip()
source_dirty = bool(subprocess.check_output(["git", "-C", str(source_tree), "status", "--porcelain", "--untracked-files=no"], text=True))
rows = []
for case, count in (("scalar", 0), ("weighted", 256)):
    for engine in ("interpreter", "native"):
        proc = subprocess.run([str(exe), engine, case, "cold", str(count), "100"],
                              check=True, capture_output=True, text=True)
        for line in proc.stdout.splitlines():
            row = json.loads(line)
            assert row["completed"] and row["kind"] == "speed"
            row["arm"] = arm
            row["binary_sha256"] = binary_sha256
            row["source_commit"] = source_sha
            row["source_dirty"] = source_dirty
            rows.append(row)
assert len(rows) == 400
with path.open("a") as f:
    for row in rows:
        f.write(json.dumps(row, separators=(",", ":")) + "\n")
print(arm, len(rows))
