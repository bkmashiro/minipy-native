# Input-name validation experiment

Candidate: `frontend::validate_inputs` checks each name against the preceding names instead of building a HashSet. The existing MAX_LOCALS=32 admission bound limits the comparison count to 496. Name validity, reserved names, duplicate rejection and error order remain unchanged. This removes the temporary hash table on nonempty input schemas, at the cost of quadratic bounded string comparisons. Long shared-prefix names may make that trade-off less favorable.

Baseline A is aa836c66555e073d33651519e99aea9c1fcbc96e plus the same `tests/input_schema_cost.rs` probe. B differs only in this validation loop. Default features, macOS ARM64, Rust 1.96.0 release. No HTTP code is enabled in these binaries. Both binaries use the same target directory; no new dependencies or worktrees.

## Measurement

Eight fresh probe processes in BAABABBA order under the shared build/performance lock. Each process creates its current-thread runtime and Provider outside the timer, then measures 101 requests for each combination of 0/2/32 inputs and tape/native. The request timer includes source preparation, execution and unit destruction. Source formatting, assertions, output logging and returned output destruction are outside it. Each source is prepared again, with no code cache. Literals vary within each cell and repeat in subsequent processes. Sample 0 is recorded but excluded from summaries. The host is unused by these scalar programs.

Build and run:

```sh
CARGO_TARGET_DIR="$PWD/target" CARGO_BUILD_JOBS=1 cargo +1.96.0 test --offline --locked --release --test input_schema_cost -- --include-ignored --nocapture
# Preserve A's test executable before rebuilding B; execute A/B in BAABABBA order:
/path/to/test-executable resident_request_cost --ignored --nocapture
python3 evidence/input-schema/summarize.py
```

`requests.jsonl.gz` contains 4,848 records, including 48 excluded first samples. `summarize.py` validates cell counts and computes per-process medians and four adjacent comparisons (B/A - 1).

## Interpretation

The initial processes show major drift even for zero inputs, where neither implementation allocates a names table. These measurements cannot establish a stable causal end-to-end speedup. In the final two pairs, tape with 2 inputs changes -10.00% / -3.49%, and 32 inputs -16.59% / -18.86%; zero-input tape changes -6.82% / +2.40%. Native changes are also affected by process drift. Do not present these as universal gains or combine them with earlier ToolEnvironment results.

Decision: revert the production candidate and retain `candidate.patch`, raw data and the tests. Focused review confirmed unchanged error/Unicode semantics, but name lengths are unbounded, so 32 long shared-prefix names can cause substantially more scanning than hashing each name once. With no stable end-to-end benefit established, removing this allocation does not justify that trade-off or a new schema-length restriction. Default and tool-json focused tests passed before reversion. Linux performance has not been measured.
