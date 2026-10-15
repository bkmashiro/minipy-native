#!/usr/bin/env python3
"""Small locked source-once string comparison of main and BG using AR's original source."""
import fcntl
import gzip
import hashlib
import json
import os
from pathlib import Path
import shutil
import statistics
import subprocess

ROOT = Path(__file__).resolve().parents[1]
PROJECT = ROOT.parents[1]
BASE = Path(os.environ.get("BH_BASE_TREE", ROOT))
BG = Path(os.environ.get("BH_BG_TREE", PROJECT / ".worktrees" / "t_cf4d1867"))
TARGET = PROJECT / "target"
OUT = ROOT / "evidence" / "bh-string-integration"
LOCK = Path.home() / ".hermes/cache/scratch/minipy-perf-build.lock"
REPS = 40


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    for tree, revision in ((BASE, "4e542619dc45a41c5264fa516c6ed8ba9d2211a8"),
                           (BG, "f17cf8ec680560c68c1e6d6e199236a83212649a")):
        actual = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=tree, text=True).strip()
        if actual != revision or subprocess.check_output(["git", "status", "--porcelain", "--", "src", "bench/src/bin/ar_contains.rs"], cwd=tree):
            raise RuntimeError(f"Expected clean benchmark source at {revision}; got {actual} in {tree}. Set BH_BASE_TREE to an isolated worktree at the pinned main revision.")
    with LOCK.open("a+b") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        bins = {}
        for name, tree in (("base", BASE), ("candidate", BG)):
            env = dict(os.environ, CARGO_TARGET_DIR=str(TARGET), CARGO_BUILD_JOBS="1")
            subprocess.run(["cargo", "+1.96.0", "build", "--offline", "--locked", "--release", "--manifest-path", str(tree / "bench/Cargo.toml"), "--features", "string-probe", "--bin", "ar-contains"], check=True, cwd=tree, env=env)
            built = TARGET / "release/ar-contains"
            dest = Path.home() / ".hermes/cache/scratch" / f"bh-{name}-ar-contains"
            shutil.copy2(built, dest)
            bins[name] = dest
        summary = {"reps": REPS, "order": ["base", "candidate", "candidate", "base", "base", "candidate"], "sha256": {k: digest(v) for k, v in bins.items()}, "source": "bench/src/bin/ar_contains.rs unchanged; natural alias + one contains; all four engines", "pairs": []}
        rows = {}
        for idx, arm in enumerate(summary["order"]):
            proc = subprocess.run([str(bins[arm]), str(REPS)], cwd=ROOT, capture_output=True, text=True, check=True)
            parsed = [json.loads(line) for line in proc.stdout.splitlines()]
            assert len(parsed) == 1 + REPS * 5 * 2 * 4
            assert parsed[0]["source_native"] == 'data = tools.fetch(batch_id)\ntext = data["message"]\nprint("error" in text)\n'
            assert all(row["kind"] == "run" for row in parsed[1:])
            rows[idx] = parsed[1:]
            with gzip.open(OUT / f"{idx}-{arm}.jsonl.gz", "wt") as fh:
                for row in parsed:
                    fh.write(json.dumps(row, separators=(",", ":")) + "\n")
        for a, b in ((0, 1), (3, 2), (4, 5)):
            cells = {}
            for mode in ("cold", "warm"):
                for engine in ("native", "tape"):
                    for case in ("short-hit", "long-hit", "long-miss", "escaped-hit", "max-miss"):
                        key = f"{engine}/{mode}/{case}"
                        x = [r["ns"] for r in rows[a] if (r["engine"], r["mode"], r["case"]) == (engine, mode, case)]
                        y = [r["ns"] for r in rows[b] if (r["engine"], r["mode"], r["case"]) == (engine, mode, case)]
                        assert len(x) == len(y) == REPS
                        base, cand = statistics.median(x), statistics.median(y)
                        cells[key] = {"base_ns": base, "candidate_ns": cand, "change_pct": round((cand / base - 1) * 100, 2)}
            summary["pairs"].append(cells)
        (OUT / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    for key in ("native/cold/short-hit", "tape/cold/short-hit", "native/cold/long-hit", "tape/cold/long-hit", "native/warm/long-hit", "tape/warm/long-hit"):
        print(key, [(p[key]["base_ns"], p[key]["candidate_ns"], p[key]["change_pct"]) for p in summary["pairs"]])
    print("binary hashes", summary["sha256"])


if __name__ == "__main__":
    main()
