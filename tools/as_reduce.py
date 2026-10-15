"""Recompute the bounded AS string/HTTP evidence from committed raw records."""
import csv
import json
from pathlib import Path
import statistics

root = Path("evidence/as-integration")
summary = {"string": [], "http": []}
groups = {}
for process in range(3):
    path = root / "string" / f"run{process}.jsonl"
    rows = [json.loads(line) for line in path.read_text().splitlines()]
    assert len(rows) == 801 and rows[0]["kind"] == "identity"
    assert sum(r["kind"] == "run" for r in rows) == 800
    counts = {}
    for row in rows[1:]:
        key = (row["case"], row["mode"], row["engine"])
        counts[key] = counts.get(key, 0) + 1
        assert row["result"] in ("True", "False")
        assert row["result"] == ("False" if row["case"] in ("long-miss", "max-miss") else "True")
        groups.setdefault(key, {}).setdefault(process, []).append(row["ns"])
    assert len(counts) == 40 and set(counts.values()) == {20}
for key, processes in sorted(groups.items()):
    assert len(processes) == 3 and all(len(v) == 20 for v in processes.values())
    summary["string"].append({"case": key[0], "lifecycle": key[1], "engine": key[2],
                              "median_process_median_ns": statistics.median(
                                  statistics.median(processes[i]) for i in range(3)),
                              "process_medians_ns": [statistics.median(processes[i]) for i in range(3)]})
http = {}
for process in range(3):
    rows = list(csv.DictReader((root / "http" / f"run{process}.csv").open()))
    assert len(rows) == 24
    assert [r["syntax"] for r in rows[:12]] == (["direct"] * 12 if process == 1 else ["assigned"] * 12)
    for row in rows:
        key = (row["syntax"], row["engine"], row["lifecycle"])
        assert row["checksum"] == str(115 * int(row["iterations"]))
        assert int(row["responses"]) == 2 * (int(row["iterations"]) + 1)
        assert row["connections"] == "1" and row["merge_copied_bytes"] == "0"
        http.setdefault(key, {}).setdefault(process, []).append(int(row["total_ns"]) / int(row["iterations"]))
for key, processes in sorted(http.items()):
    assert len(processes) == 3 and all(len(v) == 3 for v in processes.values())
    summary["http"].append({"syntax": key[0], "backend": key[1], "lifecycle": key[2],
                            "median_process_mean_ns": statistics.median(
                                statistics.mean(processes[i]) for i in range(3)),
                            "process_means_ns": [statistics.mean(processes[i]) for i in range(3)]})
assert len(groups) == 40 and len(http) == 8
(root / "new-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
print("string: 2400 runs, 40 groups; HTTP: 72 batches, 8 groups, 2*(iterations+1) replies per batch")
for row in summary["string"]:
    if row["case"] in ("short-hit", "long-miss", "max-miss") and row["engine"] != "host-bool":
        print("string", row["case"], row["lifecycle"], row["engine"], round(row["median_process_median_ns"]))
for row in summary["http"]:
    if row["syntax"] == "assigned":
        print("http", row["backend"], row["lifecycle"], round(row["median_process_mean_ns"]))
