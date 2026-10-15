#!/usr/bin/env python3
"""Reduce preserved raw loopback stdout; incomplete campaigns stay incomplete."""
from pathlib import Path
import re, statistics, sys, gzip

root = Path(sys.argv[1]) if len(sys.argv) == 2 else Path(__file__).parent
rows = []
for path in sorted(root.glob("run-*.txt*")):
    if path.suffix == '.gz':
        text = gzip.open(path, 'rt').read()
    else:
        if path.with_suffix(path.suffix + '.gz').exists():
            continue
        text = path.read_text()
    m = re.search(r"loopback_single_chunk variant=(\w+) samples=(\d+) median_ns=(\d+) p10_ns=(\d+) p90_ns=(\d+) copied_bytes=(\d+) responses=(\d+) raw_ns=([0-9,]+)", text)
    if not m:
        raise SystemExit(f"missing raw row: {path}")
    variant, count, median, p10, p90, copied, responses, raw = m.groups()
    values = [int(v) for v in raw.split(",")]
    if len(values) != int(count) or len(values) != int(responses) or int(copied) != 0:
        raise SystemExit(f"inconsistent accounting: {path}")
    ordered = sorted(values)
    expected = (ordered[len(values)//2], ordered[len(values)//10], ordered[len(values)*9//10])
    if expected != (int(median), int(p10), int(p90)):
        raise SystemExit(f"summary/raw mismatch: {path}: {expected}")
    rows.append((path.stem, variant, int(median), int(p10), int(p90)))
for row in rows:
    print(f"run={row[0]} variant={row[1]} median_ns={row[2]} p10_ns={row[3]} p90_ns={row[4]}")
for variant in sorted({r[1] for r in rows}):
    medians = [r[2] for r in rows if r[1] == variant]
    print(f"variant={variant} run_medians_ns={','.join(map(str, medians))} across_run_median_ns={statistics.median(medians):g}")
expected = [("baseline" if i % 2 == 0 else "candidate") for i in range(len(rows))]
print(f"campaign_complete={len(rows) == 4 and [r[1] for r in rows] == ['baseline','candidate','candidate','baseline']} observed_order={','.join(r[1] for r in rows)}")
