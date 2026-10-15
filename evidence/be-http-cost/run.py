"""Matched I/O-enabled MiniPy probe runs: identical fixture, alternate binaries."""
import csv
import hashlib
import json
from pathlib import Path
import statistics
import subprocess
import sys

out = Path(__file__).parent
base, candidate = map(Path, sys.argv[1:3])
identity = {
    "base_sha256": hashlib.sha256(base.read_bytes()).hexdigest(),
    "candidate_sha256": hashlib.sha256(candidate.read_bytes()).hexdigest(),
    "probe_sha256": hashlib.sha256(Path("examples/al_io_probe/support.rs").read_bytes()).hexdigest(),
    "lock_sha256": hashlib.sha256(Path("Cargo.lock").read_bytes()).hexdigest(),
    "rustc": subprocess.check_output(["rustc", "-Vv"], text=True),
    "platform": subprocess.check_output(["uname", "-a"], text=True),
    "order": ["AB", "BA", "BA", "AB"],
    "boundary": "I/O-enabled current-thread runtime, ready 1000-row scans and pooled real localhost HTTP; each process 3 batches per cell",
}
(out / "identity.json").write_text(json.dumps(identity, indent=2) + "\n")
for pair, order in enumerate(identity["order"]):
    for arm in order:
        result = subprocess.run([str((base if arm == "A" else candidate).resolve())], capture_output=True, text=True, check=True, timeout=60)
        (out / f"{pair}-{arm}.csv").write_text(result.stdout)

rows = []
for pair in range(4):
    for arm in "AB":
        path = out / f"{pair}-{arm}.csv"
        sample = list(csv.DictReader(path.open()))
        assert len(sample) == 45, (path, len(sample))
        for r in sample:
            n = int(r["iterations"])
            assert int(r["responses"]) == (2 * (n + 1) if r["workload"] == "mixed" else 0)
            rows.append(dict(r, pair=pair, arm=arm))
assert len(rows) == 360
summary = []
for key in sorted({(r["workload"], r["engine"], r["lifecycle"]) for r in rows}):
    selected = [r for r in rows if (r["workload"], r["engine"], r["lifecycle"]) == key]
    assert len({int(r["checksum"]) / int(r["iterations"]) for r in selected}) == 1
    means = {}
    for arm in "AB":
        means[arm] = []
        for pair in range(4):
            rs = [r for r in selected if r["arm"] == arm and r["pair"] == pair]
            n = sum(int(r["iterations"]) for r in rs)
            means[arm].append(sum(int(r["total_ns"]) for r in rs) / n / 1000)
    summary.append(dict(key=key, base_us=statistics.median(means["A"]), candidate_us=statistics.median(means["B"]), pair_pct=[(b / a - 1) * 100 for a, b in zip(means["A"], means["B"])]))
(out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
for s in summary:
    print(*s["key"], round(s["base_us"], 3), round(s["candidate_us"], 3), [round(x, 1) for x in s["pair_pct"]])
