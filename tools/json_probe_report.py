"""Bounded local AH probe: raw rows plus per-process summary, not a campaign platform."""
import argparse
import csv
import hashlib
import io
import json
import pathlib
import platform
import statistics
import subprocess

p = argparse.ArgumentParser()
p.add_argument("binary", type=pathlib.Path)
p.add_argument("output", type=pathlib.Path)
p.add_argument("--mode", choices=["timing", "allocation"], required=True)
p.add_argument("--runs", type=int, default=3)
a = p.parse_args()
assert 1 <= a.runs <= 3
out = a.output
out.mkdir(parents=True, exist_ok=True)
rows = []
expected = {(w, arm, "parse_run_cleanup") for w in ["few_fields", "array_1000", "repeated_16"] for arm in ["sonic_lazy", "sonic_typed", "serde_typed", "serde_dom"]}
expected.add(("array_1000", "sonic_row_slots", "parse_run_cleanup"))
expected |= {(w, arm, life) for w in ["json_few", "json_repeated", "scalar", "weighted", "stateful"] for arm in ["minipy_tape", "minipy_native"] for life in ["source_cold", "prepared"]}
for process in range(a.runs):
    r = subprocess.run([str(a.binary.resolve())], text=True, capture_output=True)
    (out / f"{a.mode}-{process}.csv").write_text(r.stdout)
    (out / f"{a.mode}-{process}.stderr").write_text(r.stderr)
    if r.returncode:
        raise RuntimeError(f"process {process} failed ({r.returncode}); see stderr")
    batch = list(csv.DictReader(io.StringIO(r.stdout)))
    repeats = 3 if a.mode == "timing" else 1
    assert len(batch) == len(expected) * repeats, (len(batch), len(expected) * repeats)
    counts = {}
    for row in batch:
        assert row["mode"] == a.mode
        key = row["workload"], row["arm"], row["lifecycle"]
        counts[key] = counts.get(key, 0) + 1
        assert int(row["iterations"]) > 0 and int(row["total_ns"]) > 0
        row["process"] = process
    assert set(counts) == expected and set(counts.values()) == {repeats}
    rows += batch
with (out / f"{a.mode}-raw.csv").open("w", newline="") as f:
    w = csv.DictWriter(f, fieldnames=list(rows[0]), lineterminator="\n")
    w.writeheader()
    w.writerows(rows)
summary = []
for key in sorted(expected):
    selected = [r for r in rows if (r["workload"], r["arm"], r["lifecycle"]) == key]
    process_means = [sum(int(r["total_ns"]) for r in selected if r["process"] == i) / sum(int(r["iterations"]) for r in selected if r["process"] == i) / 1000 for i in range(a.runs)]
    first = selected[0]
    summary.append(dict(workload=key[0], arm=key[1], lifecycle=key[2], independent_processes=a.runs,
                        median_process_mean_us=statistics.median(process_means),
                        min_process_mean_us=min(process_means), max_process_mean_us=max(process_means),
                        allocation_calls=first["allocation_calls"] or None, allocated_bytes=first["allocated_bytes"] or None,
                        prepared_bytes=first["prepared_bytes"] or None, code_bytes=first["code_bytes"] or None))
# Allocation-run clock values are diagnostics only, never mixed with performance timing.
if a.mode == "allocation":
    for row in summary:
        for field in ["median_process_mean_us", "min_process_mean_us", "max_process_mean_us"]:
            del row[field]
(out / f"{a.mode}-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
metadata = dict(mode=a.mode, independent_processes=a.runs, rows=len(rows), executable_sha256=hashlib.sha256(a.binary.read_bytes()).hexdigest(),
                platform=platform.platform(), architecture=platform.machine(), compiler=subprocess.check_output(["rustc", "--version"], text=True).strip(),
                fixture="8192-byte padding, {data:{a:17,b:23}}; 1000 rows price=i%97 quantity=i%5", parser_boundary="valid-input parse + compute + cleanup, input resident", source_boundary="resident frozen Provider result, real source parse/prepare where source_cold, Tool dispatch, JSON access, stdout and cleanup", trace=False, runtime_drivers="time only; no network in ready-provider measurement")
(out / f"{a.mode}-environment.json").write_text(json.dumps(metadata, indent=2) + "\n")
print(json.dumps(dict(mode=a.mode, successful_processes=a.runs, reconciled_rows=len(rows), expected_cases=len(expected)), indent=2))
for row in summary:
    print(row)
