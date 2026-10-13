# B: bounded performance-v2 experiment

Baseline: `564381d304be69d1fd870d4e92530c8024b7c394`. This card changes only diagnostic work in the runtime; benchmark/tools APIs are untouched. Formal Linux, throughput and process-density measurements belong to C. All measurements below ran on macOS M4 with Rust 1.96.0, not Linux. Core validation also ran with the existing Rust 1.93.1.

## Retained change: diagnostics on demand

Operation: fresh weighted-scan source through parse, lowering, JIT, producer/import, native task execution and cleanup. Representative existing generic benchmark: 0, 100 and 10,000 mixed rows, ready provider, chunk 256/4096. Primary metric: full `total_ns`, with compilation separately attributed. Baseline is the unchanged source implementation and pinned Monty benchmark at the above commit.

Replacement contract:

- Remove unconditional CLIF display/string allocation/retention and default Cranelift pass timers.
- Ordinary `compile` and `compile_with_opt` now leave `clif` empty. `compile_with_ir(source, opt)` retains the actual lowered CLIF; the demonstration's `--ir` uses it.
- `compile-timing` explicitly enables Cranelift timing; CLI `--ir` prints its report when built with that feature.
- Add one compile-time feature choice and one diagnostic branch per compilation. No scan/runtime check, cache, hash, fallback or dual execution was added. The native body, `speed` optimization setting and safety/authority contract remain unchanged. Unwind and host-arch support remain enabled.

Four serial process rounds alternated baseline/candidate order. Within each process the existing benchmark interleaves ordinary Monty, the limited parameterized helper and native controls. Raw data retains all 20 samples per cell; summary drops samples 0–4. Each native cell therefore has 60 retained observations. These are warm-process fresh source compilations, not process launch latency, and wall-clock samples, not CPU measurements.

Selected pooled medians, microseconds:

| rows / mode / chunk | baseline compile | candidate compile | baseline total | candidate total |
| --- | ---: | ---: | ---: | ---: |
| 0 / cold / 4096 | 109.312 | 96.605 | 113.146 | 100.229 |
| 100 / cold / 4096 | 109.396 | 97.291 | 113.146 | 101.230 |
| 10,000 / cold / 4096 | 111.042 | 98.563 | 134.250 | 121.833 |
| 10,000 / warm / 4096 | 0 | 0 | 20.458 | 20.292 |

Decision: keep diagnostic work elimination. Small-input cold total improves about 11%, and 10,000-row cold total about 9%. Do not claim a warm execution improvement from the roughly 0.8% difference. Machine code stays 192 bytes. Per-round 0-row cold totals range 110–125 us baseline and 98–108 us candidate; one baseline 10,000-row round rose to 201 us. Raw variation is retained, not discarded. There is no claim that the two sources represent all programs or that a helper replaces a generic compiler.

## Owner candidate: measured, not merged

Current `QuoteBuffer` freezes Vec via Box into `Arc<[Quote]>`, creating a new payload allocation and copying every row. The scratch candidate uses `Arc<Box<[Quote]>>`: retain the existing exact-size boxed allocation and allocate only a small Arc control/Box block. Reads still borrow `&[Quote]`; imports, bounds, immutable ownership and lifetime remain checked as before. One additional pointer indirection and a second live allocation replace a payload copy. Spare Vec capacity may still require shrinking during `into_boxed_slice`; the probe deliberately uses exact capacity.

Public compatibility issue: `downgrade` and `TestProvider::last_rows` return `Weak<[Quote]>` today. The candidate changes them to `Weak<Box<[Quote]>>`, and changes the explicit test observer type. This cannot silently merge. `owner-candidate.patch` is a concrete scratch patch, supplied for the orchestrator/user decision. No owner public API was changed in B.

Warm-code native-only probe: produce and touch independent rows, freeze, run the actual compiled source through ready async provider with chunk=4096, validate receipt, release task/payload. Input generation, freezing, execution and cleanup are inside total; source compilation and process launch are outside. 0, 10,000 and 1,000,000 rows, 25 samples/cell/process, four alternating rounds; retain 20 samples/cell/round. Rows use constant price=2, quantity=3 rather than the mixed generic corpus, so do not cross-compare the two probe totals.

Pooled medians, microseconds:

| rows | current freeze | candidate freeze | current total | candidate total |
| --- | ---: | ---: | ---: | ---: |
| 10,000 | 2.625 | 0.042 | 13.959 | 12.583 |
| 1,000,000 | 353.375 | 0.042 | 1,423.438 | 1,056.625 |

At one million rows the pooled total decreases about 26%; the three later round pairs independently decrease about 25–27%. First-round candidate is slower (1.793 ms versus current 1.651 ms), consistent with allocator/process state effects but not attributed conclusively. A 42 ns freeze is close to clock resolution and is not a stable latency promise. Nonempty payload pointers change under current storage and are preserved in every candidate sample.

Separate allocator probe records requested live/peak bytes via a counting System allocator, not RSS or physical pages; it is not used in timed execution measurements. Each freeze makes one new allocation:

| 1,000,000 rows | current | candidate |
| --- | ---: | ---: |
| input capacity bytes | 16,000,000 | 16,000,000 |
| peak live requested bytes above initial process base | 32,000,016 | 16,000,032 |
| frozen live requested bytes | 16,000,016 | 16,000,032 |
| live requested bytes with only Weak observer | 16,000,016 | 32 |

Important: a failed Weak upgrade proves no live strong owner, but `Weak<[Quote]>` still pins its coallocated Arc allocation. The existing weak lifetime tests do not prove deallocation or RSS release. The candidate frees boxed payload on last strong drop and retains only its 32-byte control block with Weak observers. This is relevant to `TestProvider` wait-state measurements and must be separated from allocator-retained RSS in C. The one-shot benchmark provider does not retain such a Weak.

Decision: recommend the narrow owner change if its explicit public observer type change is approved. It adds 16 requested metadata bytes, removes up to 16 MB copy/transient live bytes at the existing row limit and improves weak-only retention. Do not create a generic owner abstraction merely to hide this compatibility decision. Formal mixed-source/input and physical-memory confirmation remains C work.

## Validation, provenance and failures

- Default `cargo test --locked`: 31 tests pass (16 unit, 7 acceptance, 8 boundary). Added focused diagnostic opt-in/parity test; source/stateful test explicitly requests IR.
- `cargo clippy --all-targets --locked -- -D warnings`, plus all-features Clippy and `git diff --check`: pass.
- `cargo test --locked --features compile-timing`: 31 tests pass; actual CLI `--ir` prints lowered CLIF and a pass timing table, receipt=41, one load and one record.
- Scratch owner: all 31 tests pass including clone/Weak lifetime, little-endian extrema, empty/malformed/oversized import, overflow/short-circuit, cancellation/late result release, pre-record release and source-dependent native output. This validates compatibility-adjusted tests, not approval of the API change.
- Independently recomputed all 3,840 generic receipts and checked all 600 owner receipts, raw count and nonempty pointer identity outcomes. No runtime hashes or allocation counters were introduced into the shipped crate.
- Initial scratch owner build failed because a textual substitution also changed `TestProvider`'s separate source storage. Narrowed the patch to frozen storage and Weak observers; source provider representation is unchanged.
- Initial standalone owner run reused a stale library from a shared Cargo target directory. It is invalid and excluded. Fixed by explicitly cleaning this package before each standalone build, verified distinct binaries and pointer behavior, and reran. Invalid raw remains at `/Users/yuzhe/.hermes/cache/scratch/minipy-b-v2/invalid-shared-target-raw`; its apparent neutral result is not evidence about the owner candidate.

Evidence: [raw and scripts](b-performance-v2-evidence/), `summary.json`, `identity.json`, `verification.json` and `owner-candidate.patch`. Source/binary digests are offline provenance only. The scripts are exact machine-local probes, not new production benchmark infrastructure: copy them into a new scratch directory before running so snapshots/builds do not land in the repository. `setup.py` uses this worktree as root and `git archive` of the fixed baseline; `run.py` builds standalone probes with package cleanup and serially runs alternating rounds. The allocation probe was separately copied to snapshot `src/bin/alloc_probe.rs`, built with identical flags after package cleanup and its output saved as `raw/alloc-{diagnostics,owner}.csv`.

Executed commands:

```sh
python /Users/yuzhe/.hermes/cache/scratch/minipy-b-v2/setup.py
python /Users/yuzhe/.hermes/cache/scratch/minipy-b-v2/run.py
cargo +1.96.0 test --locked --manifest-path /Users/yuzhe/.hermes/cache/scratch/minipy-b-v2/owner/Cargo.toml --target-dir /Users/yuzhe/.hermes/cache/scratch/minipy-b-v2/test-target
cargo test --locked
cargo test --locked --features compile-timing
cargo run --locked --features compile-timing --bin minipy -- fixtures/weighted_scan.py --ir
cargo clippy --all-targets --locked --all-features -- -D warnings
```

C should cherry-pick the diagnostic/evidence commit, decide the observer API before applying the owner patch, and run its planned unified Linux campaign and one focused integrated review. No C-stage publication claim, Linux result, process density, 10-byte string result or additional optimization is asserted by B.
