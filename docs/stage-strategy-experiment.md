# I: execution reference and bounded strategy qualification

## Decision

Retain explicit typed-IR interpretation and explicit native compilation as separate caller choices. Add a feature-gated execution-only Rust reference for three fixed fixtures, plus opt-in diagnostic counts. Both runtime optimization candidates were tested and reverted. No default engine change, source cache, automatic JIT, OSR, alternate admission policy, scheduler, new language feature or unsafe execution boundary is introduced.

The fixed reference is a useful lower-cost execution comparison, not a source compiler, unknown-source backend or production replacement. It does not parse/typecheck/compile source; its cold label in reused benchmark output only means a fresh request. Never compare its near-zero preparation to native/IR/Monty preparation as an achievable universal compiler cost. No production engine calls the reference on its success path.

## Environment and boundaries

Actual existing Alpine ARM64 VM on M4, 2 vCPU / 2048 MiB, started from off exclusively. Preflight: 1868 MiB available, zero swap use, 11.7 GiB free guest disk. Native/core correctness Rust 1.93.1, benchmark Rust 1.96.0, Zig 0.15.2, Linux target aarch64-unknown-linux-musl. Cranelift and Monty versions/configuration are unchanged from H. No perf executable was available in the guest; no kernel or global settings were changed. Actual Linux timings/counters and one auxiliary macOS sample are distinguished below.

The campaign retains H's five synthetic shapes, light weights 24/8/4/2/2 and scan weights 8/16/8/4/4. Each finite 100 ms window schedules 40 arrivals, 500 us apart, permuted by (arrival*17)%40, then drains. Nominal 400 requests/s across the finite window, 2000/s within its burst. This does not establish sustainable admission or service capacity. Planned-arrival latency includes timer granularity, compilation, scan queueing and late wakes. Three rotated/reversed serial rounds; mixed 21 windows, retain samples >=3; standalone 101 requests, retain >=21. All raw, including overloaded and transition observations, are preserved.

Cold includes independent parse/typecheck/prepare, real provider allocation/freeze/import, actual execution, output, detached Tool waits and code/task teardown. Warm explicitly reuses five prepared units and includes Monty clone; it assumes no production cache hit rate. JSON and expected-output construction are outside timing; assertion is inside identically. Whole-process CPU includes scheduling and all task/provider work. Preparation and execution columns are wall subintervals, not additive CPU attribution. Ready providers still use Tokio spawn/oneshot. Delayed providers perform two actual 1000 us sleeps per scanning request. Speed binaries have neither allocation nor interpreter-count instrumentation. Hold snapshots use a distinct allocation-diagnostic binary.

There are 72,600 baseline rows and 36,300 rows per candidate (145,200 total), each verified against its command count. Baseline has 199 overloaded raw windows, including a noisy light warm Monty round; both two-engine candidate campaigns have zero. Overloaded batches remain in raw and summary. Small standalone cells have severe frequency/scheduling variability (e.g. baseline warm reference scalar CPU rounds 3.66/11.88/12.00 us), so small differences and unmatched campaign comparisons are not optimization evidence.

## Reference contract and oracle

src/fixture_reference.rs is built only with fixture-reference. Fixture enum selection is explicit in the harness, not source-string recognition or hidden compiled business replacement. Scalar, weighted and cross-row stateful programs perform signed checked arithmetic and ordered short-circuit predicates. They use the same Provider interface, QuoteBuffer import, runtime authorize_tool, spawn/oneshot detached operations, receive cancellation handling, bounded Output and chunk-yield/cumulative-row-fuel contract as the engines. QuoteBuffer is dropped at proven last use before record authorization/submission. There is no shadow baseline execution.

The reference has a different coroutine/frame representation, intentionally not claimed identical. Trace frame_bytes is a diagnostic approximation of fixed live numeric state, not the actual reference future allocation; waiting allocation and physical process measurements are the density evidence. Phase/owner bytes/remaining/index, stdout, errors and tool counts are compared independently of frame size.

New three-way native/IR/reference tests cover scalar and scan semantics, filtered overflow, arithmetic/accumulation overflow, empty input, budget exhaustion, chunk sizes 1/2/256/4096, invalid chunks, stdout capability/limits, Tool capability/call limits, provider failures, invalid input and pre-cancellation. Synchronized gated tests cover receiver drop and cancellation at load/record waits, late completion without execution resumption, last-use weak-owner release before record, and no later record after load cancellation. A first-chunk cancellation test asserts actual observed Pending, row/fuel trace and owner reclamation, never sleep-based cancellation timing. The broader existing tests continue to protect source admission, short-circuit/error order and shared-unit state.

## Same-round baseline

Microseconds. Short means scalar plus 100-row scans. Quantiles are nearest-rank pooled request values. Whole-process CPU/task is total retained-window CPU divided by all 40 tasks, not averages of shape medians.

| Mix / mode / delay us | Engine | CPU/task | Mean latency | Short p95 | Short p99 |
|---|---|---:|---:|---:|---:|
| light/mixed-cold/0 | interpreter | 588.31 | 2116.50 | 1929.58 | 2701.58 |
| light/mixed-cold/0 | monty | 1868.98 | 37949.85 | 70766.75 | 73618.07 |
| light/mixed-cold/0 | native | 688.49 | 10419.39 | 19630.42 | 23484.29 |
| light/mixed-cold/0 | reference | 155.75 | 1237.33 | 2033.45 | 2308.50 |
| light/mixed-cold/1000 | interpreter | 589.33 | 3852.33 | 7001.70 | 8294.91 |
| light/mixed-cold/1000 | monty | 1966.55 | 38946.72 | 79149.44 | 81882.34 |
| light/mixed-cold/1000 | native | 636.75 | 11075.64 | 26289.49 | 29922.37 |
| light/mixed-cold/1000 | reference | 154.12 | 3186.90 | 7054.65 | 8013.07 |
| light/mixed-warm/0 | interpreter | 518.35 | 1806.26 | 1612.66 | 2060.90 |
| light/mixed-warm/0 | monty | 2197.86 | 43665.10 | 75889.28 | 308931.10 |
| light/mixed-warm/0 | native | 176.14 | 1238.80 | 1946.58 | 2344.79 |
| light/mixed-warm/0 | reference | 159.54 | 1244.57 | 2041.17 | 2798.25 |
| scan/mixed-cold/0 | interpreter | 837.01 | 4168.26 | 2453.51 | 3001.41 |
| scan/mixed-cold/0 | monty | 3930.63 | 101526.89 | 153433.77 | 156814.22 |
| scan/mixed-cold/0 | native | 848.21 | 18883.53 | 28684.69 | 31192.26 |
| scan/mixed-cold/0 | reference | 272.95 | 1399.73 | 2344.85 | 2865.12 |
| scan/mixed-cold/1000 | interpreter | 870.40 | 7750.23 | 7629.51 | 9042.55 |
| scan/mixed-cold/1000 | monty | 3943.51 | 110490.85 | 171066.51 | 174632.82 |
| scan/mixed-cold/1000 | native | 869.77 | 22464.06 | 34139.80 | 36766.00 |
| scan/mixed-cold/1000 | reference | 280.58 | 5232.86 | 7691.84 | 8746.93 |
| scan/mixed-warm/0 | interpreter | 838.71 | 3892.30 | 2011.58 | 2353.25 |
| scan/mixed-warm/0 | monty | 3909.77 | 102869.50 | 153085.75 | 170891.40 |
| scan/mixed-warm/0 | native | 286.08 | 1370.49 | 2098.54 | 2721.97 |
| scan/mixed-warm/0 | reference | 263.40 | 1419.34 | 2393.35 | 2976.46 |

Standalone source-cold versus explicitly prepared CPU in us. Reference omits language compilation by construction. The detailed phase and per-round ledger remains in summary.json.

| Shape | Engine | Cold CPU | Cold preparation wall | Cold execution wall | Prepared CPU |
|---|---|---:|---:|---:|---:|
| scalar:0 | reference | 4.45 | 0.39 | 1.97 | 9.18 |
| scalar:0 | native | 233.55 | 225.37 | 6.26 | 3.35 |
| scalar:0 | interpreter | 52.29 | 41.62 | 6.92 | 3.45 |
| scalar:0 | monty | 54.17 | 31.75 | 20.47 | 42.58 |
| weighted:100 | reference | 14.86 | 0.48 | 11.25 | 16.10 |
| weighted:100 | native | 349.97 | 339.49 | 9.51 | 13.40 |
| weighted:100 | interpreter | 98.10 | 79.37 | 16.35 | 22.65 |
| weighted:100 | monty | 216.68 | 54.14 | 160.54 | 197.28 |
| stateful:100 | reference | 13.41 | 0.42 | 10.26 | 15.06 |
| stateful:100 | native | 416.91 | 403.25 | 12.40 | 21.58 |
| stateful:100 | interpreter | 79.63 | 65.45 | 11.73 | 27.54 |
| stateful:100 | monty | 199.47 | 44.98 | 151.99 | 185.59 |
| weighted:10000 | reference | 113.84 | 0.29 | 112.00 | 82.53 |
| weighted:10000 | native | 408.20 | 320.26 | 86.93 | 85.46 |
| weighted:10000 | interpreter | 611.33 | 36.80 | 574.32 | 335.46 |
| weighted:10000 | monty | 3878.72 | 32.53 | 3850.46 | 3777.28 |
| stateful:100000 | reference | 509.05 | 0.22 | 508.26 | 504.09 |
| stateful:100000 | native | 839.77 | 281.00 | 558.69 | 581.67 |
| stateful:100000 | interpreter | 2500.07 | 38.16 | 2464.03 | 2542.49 |
| stateful:100000 | monty | 37658.68 | 42.86 | 37663.72 | 35301.68 |

At 100k stateful rows, prepared native is 1.15x reference CPU, while IR is 5.04x and Monty 70.03x. Whole-request cold native is 1.65x reference but that ratio includes source compilation only in native. At 10k weighted rows prepared native is near reference within observed noise. At 100 rows and scalar requests, fixed setup/timer noise dominates; the aggregate mean sometimes places native ahead of reference and does not establish a faster universal execution bound. Native/reference proximity on large scans identifies a substantial provider/scheduling/fuel/output floor; there is no evidence that all time in native is numerical computation.

## Bounded interpreter attribution

Linux interpreter_counts executes 100k-row weighted/stateful programs with the same input formula, checked ops and real Tool protocol. Counts are diagnostic work inventory, not CPU shares. At chunk 256:

| Program | Recursive eval entries | Local leaves | Field leaves | Binary nodes | Statement op dispatches | Fuel reservations | Yields |
|---|---:|---:|---:|---:|---:|---:|---:|
| weighted | 880264 | 160090 | 260086 | 360086 | 440137 | 391 | 390 |
| stateful | 840681 | 113536 | 213610 | 413532 | 300320 | 391 | 390 |

The short-circuit counts (33334 weighted, 86624 stateful) are separate diagnostic events, excluded from eval-entry totals. chunk 1/4096 change reservations to 100000/25 and yields to 99999/24 with identical output and node/op counts. Thus the ordinary chunk256 path already amortizes budget checks; removing row safety/fuel or checking every field is not supported. Local reads, row-field reads and checked arithmetic occur through recursive Expr matches. Scan/Next add two dispatches per row and State.row retains one copied Quote. Counts alone cannot assign cycle shares among those activities.

Auxiliary macOS sample: actual release benchmark, interpreter stateful warm 100k, 4000 requests, sample at 1ms for 3s. After excluding the distinct dyld-startup stack, 1793 runtime samples include 1380 stacks containing State::eval (~77%). Nested recursive frames are counted once, not summed as independent time. 53 runtime samples are in the load-provider task; inlining leaves statement dispatch, fuel, row access and suspension state mixed inside Prepared::run, so those subcosts are not separately resolved. This is a hotspot localization on macOS, not a Linux CPU decomposition or controlled profiler-overhead bound. Startup sample attachment itself was invasive; macos-sampled.jsonl.gz workload observations are not used in speed acceptance. The complete raw sample is retained. No low-perturbation Linux flamegraph was obtained.

CLIF inventory shows scalar still carrying a PC load from frame+264 and resume-default Budget branch despite one resume state. Scalar pre/post backend has 4 loads and 10 blocks; it also has initial non-input local loading, frame completion stores and two host print calls. This justified only the bounded single-entry dispatch experiment, not a new scalar backend or wholesale SSA/liveness rewrite. Weighted control has 4 resume states, which must preserve dispatch and tool/chunk continuations.

## Candidate A: leaf operands bypass recursive eval prologues

Replacement: inline Int/Bool/Local/Field operand retrieval; compound operands fall back once to existing eval. This removes recursive function crossings for typed leaves, adds one leaf match and a compound fallback match, and retains left-to-right short-circuit order, checked arithmetic, owner access and every runtime guard. No old evaluator runs additionally on successful leaves. No register VM or new expression representation.

Diagnostic candidate counts show 520172 weighted / 427064 stateful leaf operand calls bypass eval. Eval entries fall to 360092 / 413617. Operation counts, output, 391 fuel reservations and 390 yields remain identical. The feature-gated counters are excluded from speed.

| Mix / mode / delay | Base CPU/task | Candidate CPU/task | Change | Base short p99 | Candidate short p99 |
|---|---:|---:|---:|---:|---:|
| light/mixed-cold/0 | 572.58 | 528.87 | -7.63% | 2561.46 | 2990.80 |
| light/mixed-cold/1000 | 588.88 | 540.04 | -8.29% | 7930.87 | 8078.30 |
| light/mixed-warm/0 | 541.50 | 467.15 | -13.73% | 2044.46 | 1991.50 |
| scan/mixed-cold/0 | 867.58 | 833.79 | -3.90% | 3467.98 | 4095.89 |
| scan/mixed-cold/1000 | 889.77 | 830.37 | -6.68% | 8721.89 | 8908.79 |
| scan/mixed-warm/0 | 840.40 | 772.23 | -8.11% | 2456.34 | 2605.87 |

A passes correctness and improves all six mixed CPU cells, with 10k weighted prepared CPU 312.47 -> 266.91 us and 100k stateful prepared 2492.79 -> 2358.00 us. Mixed CPU improves in every paired round. Ready short p99 rises in all three rounds: light base 2580.57/2644.45/2316.88 -> candidate 2999.18/2760.43/3226.04 us; scan base 3249.51/3678.39/3440.94 -> candidate 3833.18/4002.98/4823.24 us. Other tail cells have mixed directions. This repeated ready-tail signal supports conservative rejection, but three variable rounds do not establish stable causal degradation. Cold scalar CPU nominally rises 41.17 -> 68.89 us and small prepared scans are inconsistent; timer/frequency variation is material and preparation changes are not explained by leaf code. Retain the CPU gain as a positive result and revert pending stronger short-task attribution, without requiring every cell to improve. This does not prove leaf specialization can never work. No retry campaign or third candidate is added.

## Candidate B: eliminate recovery dispatch for one actual resume state

Replacement: when lowering's actual resumes inventory has only its initial entry, jump directly to start instead of loading frame.pc and constructing/emitting a Switch/default Budget dispatch. Adds one preparation-time len test, no second AST scan, no runtime guard, no fallback execution. All input/authority/overflow/fuel/cancellation/output checks remain. Multi-resume scans/tools retain their original switch.

Scalar CLIF loses one load/default branch and one optimized block; native code shrinks 264 -> 236 B in auxiliary macOS inventory. Weighted CLIF/control is identical. Correctness passes. This removes real work but cannot pay down the broader backend compilation cost by itself.

| Mix / mode / delay | Base CPU/task | Candidate CPU/task | Change | Base short p99 | Candidate short p99 |
|---|---:|---:|---:|---:|---:|
| light/mixed-cold/0 | 704.96 | 702.69 | -0.32% | 24184.83 | 23697.58 |
| light/mixed-cold/1000 | 697.13 | 722.86 | +3.69% | 29622.32 | 30302.80 |
| light/mixed-warm/0 | 170.41 | 177.39 | +4.10% | 2285.97 | 2191.76 |
| scan/mixed-cold/0 | 871.94 | 872.40 | +0.05% | 31268.79 | 31085.78 |
| scan/mixed-cold/1000 | 878.04 | 888.84 | +1.23% | 35710.09 | 35817.14 |
| scan/mixed-warm/0 | 292.18 | 302.51 | +3.53% | 2668.45 | 2756.09 |

B's standalone cold scalar nominal improvement (288.19 -> 203.03 us) is not confirmed by mixed CPU. Unchanged scan-source controls also show large cold preparation changes, proving environment/order variability can dominate small-source standalone comparisons. Light delayed CPU and both warm mixes regress. Warm scalar and weighted10k regress, with variable per-round signs elsewhere. Reject and revert. Exact rejected code patches and every raw observation remain available. Production src/codegen.rs is byte-identical to the parent.

## Waiting density and reclamation

Separate gated allocation-diagnostic process per engine/mix, all 40 arrivals submitted together. Scalars finish; 16 light / 32 scan tasks wait. Live requested increments are relative to fresh startup. Quote payload at load wait is 3,539,200 B/light and 7,078,400 B/scan. No typed Quote payload remains at record wait. Monty-clear is the source-level last-use list-clear secondary; retain it as a stronger density comparison. These are per-process snapshots, not a service task-count upper limit. JIT executable mappings are outside requested allocation counts, hence RSS/PSS alongside allocator values.

| Mix | Engine | Load increment B | Record increment B | Reclaimed increment B | Record RSS B | Record PSS B | Sampled high-water RSS B |
|---|---|---:|---:|---:|---:|---:|---:|
| light | native | 3721082 | 181436 | 28584 | 5451776 | 5443584 | 9003008 |
| light | interpreter | 3625780 | 86134 | 4098 | 3534848 | 3526656 | 7090176 |
| light | reference | 3593332 | 53686 | 4098 | 2297856 | 2289664 | 5849088 |
| light | monty | 4521660 | 27374494 | 4442 | 39235584 | 39227392 | 77357056 |
| light | monty-clear | 4521884 | 26206590 | 4442 | 37486592 | 37478400 | 81256448 |
| scan | native | 7381104 | 301874 | 29964 | 5697536 | 5689344 | 12800000 |
| scan | interpreter | 7203494 | 124264 | 4226 | 3584000 | 3575808 | 10690560 |
| scan | reference | 7148838 | 69608 | 4226 | 2330624 | 2322432 | 9433088 |
| scan | monty | 8774686 | 54480352 | 4442 | 71811072 | 71802880 | 132685824 |
| scan | monty-clear | 8775134 | 52144544 | 4442 | 68317184 | 68308992 | 129753088 |

Reference record increments are 53,686 B/light and 69,608 B/scan versus interpreter 86,134/124,264 and native 181,436/301,874. Much of this gap is representation/code ownership. Native scratch/mappings and prepared code are not simply live numeric state. All engines reach reclaimed snapshots; residual requested runtime/harness/JIT capacities are not leak claims. Native/IR keep code to final print under current script lifecycle. Optimizing that lifetime or per-task independent preparation would require another bounded hypothesis, not dropping allocations from the timed boundary.

## Architecture recommendation and stop

Keep source admission/typechecking as the authority boundary and explicit typed IR as the shared semantic representation. For source-cold tiny scripts, explicit IR avoids most native preparation; for caller-known reuse and long scans, native approaches the fixed-reference execution floor and substantially outperforms IR. The light/scan mixes expose different cold CPU and queueing tradeoffs; no source-independent row threshold or universally superior default follows. Reference cannot replace unknown-source compilation. Python-like syntax is not full Monty compatibility.

The highest unresolved IR work is expression/operator dispatch, not chunk256 fuel checks. A simple leaf-call elimination produces CPU savings but did not pass this campaign's tail/small-request gate. Further work would need more stable short-request attribution or a representation-level hypothesis with new costs/oracles; neither is authorized as an additional campaign here. Native's remaining tiny-source burden remains backend preparation (consistent with E), while its large-scan execution is already close to the host floor. The one-entry entry simplification is too low leverage in the actual mixture. Stop after two experiments, retain no neutral/negative production optimization, and keep explicit choices without automatic counters-driven tiering.

## Reproduction and verification

Run tools/summarize_stage.py to recompute all summary tables from compressed raw. tools/stage_campaign.py preserves H boundaries, refuses to overwrite historical raw, and records actual commands/source hashes/binary hashes/preflight/postflight. Candidate patches are evidence, not retained alternatives. For reproducible candidate builds, apply the relevant patch to this final diagnostic/reference tree; run cargo fmt first. Interpreter patch includes the optional counter hook, disabled for speed. Source HEAD in identity names parent ebc5002 plus listed modified-source hashes, not the final commit hash; final publication identity belongs in the task handoff. Untracked diagnostic/reference files are covered by source_sha256, not git diff alone.

Build commands:

    cargo +1.96.0 zigbuild --release --locked --manifest-path bench/Cargo.toml --target aarch64-unknown-linux-musl --bin interpreter_ab
    # copy to target/i-{base,interpreter-candidate,native-candidate}-speed-linux
    cargo +1.96.0 zigbuild --release --locked --manifest-path bench/Cargo.toml --target aarch64-unknown-linux-musl --bin interpreter_ab --features allocation-diagnostics
    # copy base to target/i-base-memory-linux
    python3 tools/stage_campaign.py HOST KEY KNOWN_HOSTS base
    python3 tools/stage_campaign.py HOST KEY KNOWN_HOSTS interpreter-candidate
    python3 tools/stage_campaign.py HOST KEY KNOWN_HOSTS native-candidate
    python3 tools/summarize_stage.py
    cargo +1.96.0 zigbuild --release --locked --features interpreter-profile --example interpreter_counts --target aarch64-unknown-linux-musl
    # execute counts on actual guest for weighted/stateful and chunks 1/256/4096
    cargo run --release --locked --features compiler-profile --example stage_inventory
    cargo test --release --locked --all-features
    cargo clippy --release --locked --all-targets --all-features -- -D warnings
    cargo fmt --check

Timing raw, memory snapshots, source/toolchain/binary hashes, precise patches, counter outputs and before/after CLIF are in bench/results/i-stage. Auxiliary sample commands/binary identities and actual correctness run logs are recorded separately there. Native and IR production execution remain unchanged except feature-gated diagnostic hooks. Linux correctness is actual guest execution, not a cross-build assertion. Final counts/tests/review/publication are recorded in the task handoff and experiments checkpoint.
