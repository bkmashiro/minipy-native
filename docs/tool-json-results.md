# AH bounded results: static Tools and borrowed JSON

## Decision

Ship the opt-in field-oriented Tool/JSON/HTTP slice for review. Do not expand it into a lazy array language runtime: both typed decoders win decisively on the full-array fixture. Keep per-task repeated-field caches because uncached repeated lazy reads add work and allocations. Native has no useful execution-speed advantage over tape on these small JSON scripts, and fresh preparation costs substantially more. No performance superiority claim or website update follows from this experiment. Native JSON leaves still call the shared runtime helper: borrowed parsing, prefix/leaf cache lookup, integer conversion and error propagation remain execution costs. JSON helper crossings and branches can dominate these short sources, reducing the possible native advantage. The helper boundary is a deliberate bounded implementation choice, not a zero-cost path; no source fusion or backend redesign was attempted in this close-out.

## Host and boundaries

Apple M4, macOS 26.4.1 arm64, rustc 1.93.1, release profile with debuginfo. Pinned sonic-rs 0.5.7, serde_json 1.0.154, Tokio 1.48.0. Local only; no WSL/AF compute or artifact touched. Existing target cache and CARGO_BUILD_JOBS=1 were used. Disk available fell from roughly 14 GiB to 11 GiB during builds; no dataset or toolchain was downloaded.

Timing build has no trace or allocation-profiler feature. Separate allocation diagnostic build has a thread-local GlobalAlloc counter. Allocation bytes are total requested allocation/reallocation bytes, not resident/peak memory, and its clock values are not performance results. Counters describe one actual invocation per cell, not a statistical estimate. Missing counters in timing CSV stay blank, never zero.

Three independent timing processes, each with three sequential batches per cell. Tables report the median of the three process means (microseconds/op). Parser cases use 1000 requests/batch, arrays use 100; source-cold uses 20, prepared uses 100. These short batches are noisy on an interactive laptop; no confidence interval or percentile claim. One correctness warm-up runs outside each measured batch. Execution checks and stdout consumption/cleanup are inside the source measurement, so these values are not generated-code-only timings.

Parser input is already resident: an 8192-byte unused padding string plus data.a=17/data.b=23, or 1000 valid unique-key rows (price=i%97, quantity=i%5). All arms compute the same checksum. Lazy parsing and typed/DOM parsing deliberately have different invalid/duplicate-input semantics, so equal valid results do not establish full semantic equivalence. Direct sonic_lazy has no cross-call field cache. The row-slots candidate has one array cursor and one object cursor per row, resets two reused scalar cells, and does not retain decoded rows or a DOM.

Real MiniPy source cases use shared resident Bytes and a frozen 1000-row Quote owner in a ready asynchronous Provider. The actual parser, frontend, native/tape compiler, detached Tool dispatch, borrowed JSON helpers, print and cleanup are exercised. Source-cold means a fresh unit each invocation, but host/runtime, fixture and compiler TLS scratch are warm. Prepared excludes source preparation and its retained unit. These are not HTTP response-latency numbers. Real network behavior is separately proved by localhost HTTP tests.

Ready-provider runs explicitly enable only Tokio's time driver. This keeps the old and new runtime controls aligned. Full I/O-enabled observations remain under `evidence/tool-json/io-enabled/`; driver-control CSVs isolate the difference using the same new executable.

## Parser timing and allocation diagnostics

| Workload | Arm | median process mean us | allocations/op | allocated bytes/op |
|---|---|---:|---:|---:|
| array_1000 | serde_dom | 120.667 | 3009 | 710408 |
| array_1000 | serde_typed | 33.765 | 9 | 32704 |
| array_1000 | sonic_lazy | 158.162 | 2000 | 256000 |
| array_1000 | sonic_row_slots | 127.673 | 3000 | 141000 |
| array_1000 | sonic_typed | 33.507 | 9 | 32704 |
| few_fields | serde_dom | 0.952 | 7 | 9469 |
| few_fields | serde_typed | 0.552 | 0 | 0 |
| few_fields | sonic_lazy | 0.146 | 2 | 256 |
| few_fields | sonic_typed | 0.493 | 0 | 0 |
| repeated_16 | serde_dom | 0.950 | 7 | 9469 |
| repeated_16 | serde_typed | 0.539 | 0 | 0 |
| repeated_16 | sonic_lazy | 2.209 | 32 | 4096 |
| repeated_16 | sonic_typed | 0.493 | 0 | 0 |

Few-field lazy access is cheaper on this resident valid fixture, but still allocates parser scratch. Repeating two uncached field paths 16 times increases lazy allocations from 2 to 32. Array row slots improve on two per-row getters but are still much slower than the roughly 33 us typed decode, with 3000 allocations versus 9. That negative result is why no new source array/container path was promoted.

## Real source / backend lifecycle

| Source | Backend | cold prepare+run us | prepared run us | retained tape bytes / native code bytes |
|---|---|---:|---:|---:|
| json_few | minipy_tape | 9.571 | 0.869 | 976 |
| json_few | minipy_native | 84.615 | 0.822 | 424 |
| json_repeated | minipy_tape | 71.594 | 0.957 | 8440 |
| json_repeated | minipy_native | 467.808 | 0.929 | 2196 |
| scalar | minipy_tape | 7.410 | 0.224 | 626 |
| scalar | minipy_native | 64.404 | 0.226 | 264 |
| weighted | minipy_tape | 25.108 | 8.083 | 1122 |
| weighted | minipy_native | 146.676 | 1.905 | 552 |
| stateful | minipy_tape | 28.442 | 7.115 | 1146 |
| stateful | minipy_native | 162.621 | 2.018 | 576 |

JSON repeated source reads each of two leaves 16 times and uses the production prefix/integer cache. Prepared native allocation diagnostics are identical for few/repeated: 13 calls, 1420 bytes, while execution remains sub-microsecond on the time-only ready-provider configuration. This supports the bounded cache, not a general native speed claim. Tape preparation is 976 bytes for few fields and 8440 for the intentionally expanded repeated source; native executable code is 424 and 2196 bytes respectively. Tape logical bytes include the path/call table but exclude allocator/Rc overhead. Native code_bytes excludes its JITModule, path table and allocation/page overhead; these are separate metrics, not an equal memory comparison.

## Old fast-path comparison and discovered reactor effect

The same standalone fastpath_probe source was compiled at baseline c3dff6a (default features) and current implementation (tool-json enabled). Three independent process pairs alternate arm order. Checksums match; native generated code lengths remain exactly 264/552/576 bytes for scalar/weighted/stateful. Tape units add 32 bytes of optional-plan/liveness header when tool-json is compiled in, but allocate no JSON cache, cursor, owner or HTTP client for these programs. Default-feature builds have no such header additions.

| Workload | Backend | lifecycle | baseline us | tool-json enabled us | new/base |
|---|---|---|---:|---:|---:|
| scalar | native | source_cold | 69.719 | 83.494 | 1.198 |
| scalar | native | prepared | 0.168 | 0.222 | 1.324 |
| scalar | tape | source_cold | 8.800 | 11.692 | 1.329 |
| scalar | tape | prepared | 0.182 | 0.235 | 1.289 |
| weighted | native | source_cold | 147.567 | 154.665 | 1.048 |
| weighted | native | prepared | 1.911 | 2.001 | 1.047 |
| weighted | tape | source_cold | 25.519 | 30.733 | 1.204 |
| weighted | tape | prepared | 8.170 | 8.856 | 1.084 |
| stateful | native | source_cold | 163.335 | 162.662 | 0.996 |
| stateful | native | prepared | 2.026 | 2.001 | 0.987 |
| stateful | tape | source_cold | 29.685 | 29.363 | 0.989 |
| stateful | tape | prepared | 7.153 | 7.147 | 0.999 |

This is a small regression observation, not proof of zero overhead. Scalar samples show ~53 ns more prepared execution in the feature-enabled pair; weighted prepared is ~5% native / ~8% tape higher, while stateful is essentially unchanged. Source-cold scatter also reaches ~20–33% in some small cells. No semantic or assembly-size regression was observed, but these results do not justify claiming old fast paths are universally equally fast with the optional feature enabled. No new optimization was added to chase these noisy small differences.

The first run used Builder::enable_all in both binaries. Adding tokio/net made that same source enable an I/O reactor only in the new binary, so the runtime controls were not equal. Ready Quote programs with three chunk fairness yields rose by roughly 14 us. A same-binary driver control confirmed native weighted 18.081 us with I/O vs 1.911 us time-only, native stateful 16.095 vs 2.027 us. It does not establish a precise universal 14 us constant, but it identifies the driver as the large effect. Raw observations are retained. The final comparison explicitly uses time-only for both ready-provider arms. Actual HTTP runs require the I/O driver and will still pay its scheduling costs; no fairness yield, cancellation check or budget was removed to hide that. Host applications without HTTP should avoid enable_all when they only require timers.

## Semantics, ownership and real HTTP evidence

10 new integration tests execute both backends: static ID/arg binding, argument source order, accessed missing/null/type/fraction/i64 range errors, first duplicate key and escaped keys, short-circuit skipped errors, malformed selected subtree/UTF-8, lazy missing outer terminator, task permissions/call counts, aggregate bytes, alias statement errors, actual await with live subview, last-use/branch release, cancellation while retaining a root and detached late owner drop. The Bytes::from_owner test checks identity of the original allocation's pointer. Native Frame remains 328 bytes.

4 real TCP/HTTP tests: native+tape produce `27\n` through fetch -> JSON integer calculation -> publish, four operations reuse one pooled connection and record zero merge-copy bytes; two DATA chunks merge once and record 23 copied bytes; Content-Length/chunked body limits, HTTP status and timeout are errors; cancellation returns while the semaphore permit remains held until the HTTP operation finishes, rejecting a second admitted operation until completion. HTTP/TLS internal copies are outside copied_bytes and were not claimed zero.

The existing all-feature suite has 72 tests; with the 14 added tests the total is 86. Default-feature suite and strict all-target clippy are also run. The pre-existing wasm32-unknown-unknown + debug-runtime library build remains checked. No HTTPS server, redirect campaign, Linux/WSL measurement, browser JSON extension, full-document-validation promise or distinct-sibling object cursor is delivered. Those boundaries remain explicit in the design.

## Files and reproduction

Canonical raw/summary/environment files: evidence/tool-json/{timing,allocation}-raw.csv, *-summary.json, *-environment.json and fastpath-raw.csv/fastpath-summary.json. Driver discovery data: io-enabled/ and driver-control-{io,time}.csv. Runner accounting reconciles 297 timing rows over 33 cells, 33 allocation rows, and 72 regression rows over 12 cells; all scheduled cases completed. The allocation source count is one diagnostic process, not three.

Commands, with existing target cache selected and one build job:

    cargo test --all-features
    cargo test
    cargo clippy --all-features --all-targets -- -D warnings
    cargo check --lib --target wasm32-unknown-unknown --features debug-runtime
    cargo build --release --example json_probe --features tool-json
    python3 tools/json_probe_report.py <release-json-probe> <output-dir> --mode timing --runs 3
    cargo build --release --example json_probe --features json-allocation-profile
    python3 tools/json_probe_report.py <diagnostic-json-probe> <output-dir> --mode allocation --runs 1

For the fast-path comparison, build the identical examples/fastpath_probe.rs at c3dff6a default features and the new revision with tool-json, preserve them as evidence/tool-json/fastpath-{base,new}, then run tools/fastpath_probe_report.py. Its ready-provider default is time-only; set MINIPY_IO_DRIVER=1 to repeat the within-new-binary driver observation. The baseline checkout was an isolated task worktree; main source and AF fixtures were never changed. Generated executables are not committed or included as deliverables.
