# AD formal v2 evidence

Only `ad-corrected-json-prefork-v2` is formal. `import-summary.json` contains
60 speed, 16 mixed, 51 memory and 12 first-request cells, each three independent
process rounds. `replacement.json` is the exact AE import contract.

`raw.jsonl.gz` includes all 12,069 rows, including warmup and initialization.
Original 300 per-command gzip files remain on WSL; their hashes are preserved
in `raw-identity.json`. Source/binary/Guest identity is `identity.json`.

Replay without the WSL host, from the repository root:

    python3 tools/ad_reduce.py --replay bench/results/ad-wsl

This replays all accounting and every aggregate value, but does not pretend to
reverify remote binary/Guest paths. Full method, results, boundaries and excluded
pilot are in `docs/corrected-prefork-benchmark.md`.

Memory wait deltas use each row's `baseline_phase` and precomputed paired
`incremental_pss_bytes`, not an unconditional controller-startup subtraction.
Fork waiting baselines contain the same living child PIDs. Counts 1/8/32 support
an intercept-allowed KiB/task fit with existing data; no further benchmark is
needed. JSON response byte counts are not object-owner logical bytes.

Production src/Cargo, Pysolate runtime and the website were not modified.
Legacy other-engine measurements keep their original runtime/cohort identities.
