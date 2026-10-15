"""Bounded three-pair regression runner for unchanged fastpath_probe source.

Usage: python3 tools/as_pairs.py BASE DEFAULT FEATURE OUTPUT_DIR
All arms use the same time-only Tokio driver; separate I/O controls use
MINIPY_IO_DRIVER=1 and are not compared as optimization arms.
"""
import csv
import hashlib
import io
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys


def run(binary, io_driver=False):
    env = os.environ.copy()
    env.pop("MINIPY_IO_DRIVER", None)
    if io_driver:
        env["MINIPY_IO_DRIVER"] = "1"
    output = subprocess.check_output([str(binary)], env=env, text=True)
    rows = list(csv.DictReader(io.StringIO(output)))
    assert len(rows) == 12
    for row in rows:
        assert int(row["checksum"]) != 0
    return rows


def main():
    base, default, feature, directory = map(Path, sys.argv[1:])
    directory.mkdir(parents=True, exist_ok=True)
    binaries = {"base": base, "default": default, "feature": feature}
    with (directory / "identity.json").open("w") as file:
        json.dump({key: {"path": str(path), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
                   for key, path in binaries.items()}, file, indent=2)
    raw = []
    for pair in range(3):
        for arm in (["base", "default", "feature"] if pair % 2 == 0 else
                    ["feature", "default", "base"]):
            for row in run(binaries[arm]):
                raw.append({"pair": pair, "snapshot": arm, "driver": "time", **row})
        # Same feature binary controls true I/O cost, never substitute the time driver for HTTP.
        for row in run(feature, io_driver=True):
            raw.append({"pair": pair, "snapshot": "feature", "driver": "io", **row})
    with (directory / "raw.csv").open("w", newline="") as file:
        writer = csv.DictWriter(file, fieldnames=list(raw[0]), lineterminator="\n")
        writer.writeheader()
        writer.writerows(raw)
    summary = {}
    for row in raw:
        key = (row["workload"], row["arm"], row["lifecycle"], row["driver"], row["snapshot"])
        summary.setdefault(key, {}).setdefault(row["pair"], []).append(row)
    values = []
    for key, pairs in sorted(summary.items()):
        assert len(pairs) == 3 and all(len(rows) == 1 for rows in pairs.values())
        samples = [r[0] for _, r in sorted(pairs.items())]
        assert len({r["checksum"] for r in samples}) == 1
        assert len({r["code_bytes"] for r in samples}) == 1
        assert len({r["prepared_bytes"] for r in samples}) == 1
        per_op = [int(r["total_ns"]) / int(r["iterations"]) for r in samples]
        values.append({"workload": key[0], "backend": key[1], "lifecycle": key[2],
                       "driver": key[3], "snapshot": key[4], "median_ns": statistics.median(per_op),
                       "pair_ns": per_op, "checksum": samples[0]["checksum"],
                       "prepared_bytes": samples[0]["prepared_bytes"],
                       "code_bytes": samples[0]["code_bytes"]})
    (directory / "summary.json").write_text(json.dumps(values, indent=2) + "\n")
    print(f"{len(raw)} rows, {len(values)} groups, 3 independent AB/BA/AB pairs")
    for item in values:
        if item["driver"] == "time" and item["lifecycle"] == "source_cold":
            print(item["workload"], item["backend"], item["snapshot"], round(item["median_ns"]),
                  item["prepared_bytes"], item["code_bytes"])


if __name__ == "__main__":
    main()
