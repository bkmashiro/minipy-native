"""Supplemental process-inclusive CPU control, not per-request attribution."""
import csv
import hashlib
import io
import json
from pathlib import Path
import resource
import statistics
import subprocess
import sys

out = Path(__file__).parent
base, candidate = map(Path, sys.argv[1:3])
rows = []
for pair, order in enumerate(["AB", "BA", "BA", "AB"]):
    for arm in order:
        binary = base if arm == "A" else candidate
        before = resource.getrusage(resource.RUSAGE_CHILDREN)
        result = subprocess.run([str(binary.resolve())], capture_output=True, text=True, check=True, timeout=60)
        after = resource.getrusage(resource.RUSAGE_CHILDREN)
        batches = list(csv.DictReader(io.StringIO(result.stdout)))
        assert len(batches) == 45
        assert all(int(r["responses"]) == (2 * (int(r["iterations"]) + 1) if r["workload"] == "mixed" else 0) for r in batches)
        rows.append(dict(pair=pair, arm=arm, sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                         user_s=after.ru_utime - before.ru_utime,
                         system_s=after.ru_stime - before.ru_stime))
summary = {arm: statistics.median(r["user_s"] + r["system_s"] for r in rows if r["arm"] == arm) for arm in "AB"}
print(summary, "paired percentage", [round(((b["user_s"] + b["system_s"]) / (a["user_s"] + a["system_s"]) - 1) * 100, 2) for a, b in ((next(r for r in rows if r["pair"] == i and r["arm"] == "A"), next(r for r in rows if r["pair"] == i and r["arm"] == "B")) for i in range(4))])
(out / "process-cpu.json").write_text(json.dumps(dict(rows=rows, process_cpu_median_s=summary), indent=2) + "\n")
