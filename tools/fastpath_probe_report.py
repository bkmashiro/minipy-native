"""Same probe at the baseline and AH snapshots; three small alternating process pairs."""
import csv
import io
import json
import pathlib
import statistics
import subprocess

out = pathlib.Path("evidence/tool-json")
rows = []
for process in range(3):
    for arm in (["base", "new"] if process % 2 == 0 else ["new", "base"]):
        r = subprocess.run([str((out / f"fastpath-{arm}").resolve())], capture_output=True, text=True, check=True)
        (out / f"fastpath-{arm}-{process}.csv").write_text(r.stdout)
        batch = list(csv.DictReader(io.StringIO(r.stdout)))
        assert len(batch) == 12
        for row in batch:
            row["snapshot"] = arm
            row["process"] = process
            rows.append(row)
with (out / "fastpath-raw.csv").open("w", newline="") as f:
    w = csv.DictWriter(f, fieldnames=list(rows[0]), lineterminator="\n")
    w.writeheader()
    w.writerows(rows)
summary = []
for work in ["scalar", "weighted", "stateful"]:
    for engine in ["native", "tape"]:
        for lifecycle in ["source_cold", "prepared"]:
            def select(arm):
                return [r for r in rows if (r["workload"], r["arm"], r["lifecycle"], r["snapshot"]) == (work, engine, lifecycle, arm)]
            old, new = select("base"), select("new")
            assert len(old) == len(new) == 3
            assert {r["checksum"] for r in old} == {r["checksum"] for r in new}
            med = lambda rs: statistics.median(int(r["total_ns"]) / int(r["iterations"]) / 1000 for r in rs)
            row = dict(workload=work, engine=engine, lifecycle=lifecycle, independent_pairs=3,
                       base_us=med(old), new_us=med(new), ratio_new_over_base=med(new) / med(old),
                       base_prepared_bytes=int(old[0]["prepared_bytes"]), new_prepared_bytes=int(new[0]["prepared_bytes"]),
                       base_code_bytes=int(old[0]["code_bytes"]), new_code_bytes=int(new[0]["code_bytes"]))
            summary.append(row)
(out / "fastpath-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
print("72 raw rows, 12 cells, 3 alternating independent process pairs, all checksums reconcile")
for row in summary:
    print(row)
