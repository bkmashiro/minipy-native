# Experiments

## CT: matched Tool schema borrowing (2026-10-16)

Retain preparation-only declaration/result-schema borrowing with an owned final Plan; no runtime Environment retention or public API change. Independent allocation diagnosis removes schema-width growth (typed JSON, four calls: 179/675 → 155/155 allocations for 2/64 leaves). The fixed no-PGO full-request matrix completed 192 processes/9216 requests. Wide-schema tape B/A 0.639–0.782 is a synthetic positive signal; the first narrow tape cell drifted/regressed (2.249), native is not consistently faster, and no broad latency/no-regression claim is made. Stop without replacement sampling or expanded scope. Both schema variants outlive Environment on tape/native; fourteen structured diagnostics match the fixed CS baseline exactly. Default/tool-json/release tool-json suites passed 79/133/133 tests. Full boundaries, commands, identities and limits: [CT](ct-schema.md), `evidence/ct-schema/`. Compiler-profile registration timing correction is separate (`a5efb03` after borrowing `d1dc105`), outside these formal binaries; two diagnostic tests and ten ordinary-release focused regressions passed. One successful native Hermes gpt-6-luna/openai-codex [review](ct-review.md) approved fixed a5efb03 with no required changes, independently reproducing the summary in memory. The failed CLI model route performed no review. Integrate the reviewed source unchanged into main, retain no-PGO release and stop; operator review controls the existing CU dependency.

## AV: main integration of the opt-in AS slice (2026-10-15)

Merge `integration/as-features` (66aef08) into main after the AF capacity commits at 5e04767, retaining both parents. The AF IPC and interpreter benchmark adapters and `tools/ad_pysolate.go` are unchanged from the first parent; no AF campaign or AS timing was rerun. The merge had no file conflicts and introduces no new production implementation beyond AS. Local offline/locked checks: all-feature and no-default-feature runtime tests, all bench targets on Rust 1.96, five AF reducer unit tests, existing Wasm debug-runtime and no-feature library checks. The all-feature suite includes native/tape real localhost HTTP JSON string execution. `tool-json` is opt-in (`cargo test --features tool-json`); default remains unchanged. As before, `tool-json` does not build on WASI with Tokio's network feature, and neither a Linux run nor remote API/TLS performance is established by this merge.

## AS: integrated opt-in Tool/JSON/string slice (2026-10-15)

AH→AJ→AL→AN→AO→AR ancestry was merged once into `integration/as-features` from main c3dff6a. Only AH/AJ/AN/AR add runtime/frontend behavior; AL/AO rejected performance candidates remain withdrawn. The old scalar/weighted/stateful source-cold matched default and opt-in controls show no stable substantial old-path regression in three AB/BA/AB process pairs. The actual I/O-enabled Host retains its ~14 µs ready scan scheduling overhead on macOS; it was not hidden by using a time-only driver for HTTP. New JSON few-field, string hit/miss against ordinary Monty and a Host-bool comparator, and a pooled real localhost fetch→field→publish→print chain were measured in bounded three-process runs. Short string prepared execution can win, while cold native preparation and long misses lose; retention is for a narrow opt-in capability, not a generalized JIT speed claim. A JSON State grows 80→128 B even for non-string JSON plans, while scalar/Quote-only tasks allocate no JSON State. Default/Wasm checks pass; `tool-json` on WASI hits pinned Tokio's `net` restriction. Exact raw paths, costs, comparisons, source identities, commands and caveats: [AS integration](as-integration.md) and `evidence/as-integration/`. The focused review approved AS; AV records the subsequent main integration above.

## AD: corrected JSON/prefork measurement (2026-10-14)

Formal v2 completed 300/300 commands, 11,769 measured rows + 300 initialization rows, three independent processes per cell. Pysolate source reading/adaptation and all oracles are outside request/window timing; user Python compilation remains inside. CPython is a real import-prewarmed per-request fork/exit/waitpid path, not a worker pool, using the same Go Quote producer and JSON/dict/SimpleNamespace representation. Native CPU reads the kernel parent clock plus reaped-child counters, without a sampling RPC. The full previous pilot is preserved and not pooled; no further campaign is scheduled.

COW/prefork scalar wall is 3.092/3.221 ms; weighted 10k is 139.313/31.437 ms, weighted 100k 1403.057/223.674 ms. Mixed COW short p99 is lower, while prefork CPU and finite throughput are better. Whole-tree memory and matched per-row empty baselines are exported; counts 1/8/32 suffice for an intercept-allowed KiB/task fit. Production src/Cargo and upstream Pysolate runtime are unchanged by this task; other engines keep original cohort/runtime identities.

See [brief](ad-benchmark-brief.md), [methods/results](corrected-prefork-benchmark.md), and `bench/results/ad-wsl/{raw.jsonl.gz,import-summary.json,replacement.json}`. Offline verification: `python3 tools/ad_reduce.py --replay bench/results/ad-wsl`. Full commands, identities, boundaries and first-phase additivity are in the export. Website import/publication is the pre-created AE task, not this implementation phase.

Status: real source-to-native execution and synthetic measurements recorded. Independent review of 87e0131 found no confirmed implementation blocker, but identified the fixture-specific helper contract gap. The bounded generic-helper correction below is implemented and measured; same-card approval is pending. These results do not establish production workload or sandbox-equivalence claims.

## Questions

1. Can a small typed frontend generate a genuine native loop with no per-row object allocation, Host callback or intermediate array?
2. How much does one-shot compilation add, and for which input sizes/user logic does native execution repay it?
3. Can the task release the response before its second I/O wait while preserving cancellation and late-completion correctness?
4. Where are CPU, memory, compile time and code size actually spent? Can a redesign eliminate work rather than merely tune it?

## Required comparisons

Use pinned Monty source/release identity. Compare ordinary Monty and Monty with the same efficient Host data layer and a predeclared finite set of general helpers. Do not force Monty through Python/IPC per field. If helpers accept an arbitrary execution plan, include its validation/compilation/execution cost.

Begin with the quote filter/aggregate fixture and add one bounded cross-row stateful/branching variant; simple aggregate results alone cannot decide the value of all compiled user logic. Parameters/conditions must change generated IR, not dispatch to a fixture-specific handwritten solution. Handwritten Rust with equivalent checks and ABI is an optional ceiling, not an equal sandbox claim.

Primary path: unique source execution, empty user-code cache, include parse/check/lower/codegen/install/execute/cleanup. Separately measure explicit compile-once/run-many without claiming an observed reuse rate. Include producer construction and any conversions. Delays are controls, not representative production wait distributions.

Measure small to large inputs, short/long scan mixtures and actual async completion. Count returned results and task errors. Report CPU, wall latency including queueing, compiled code, task/frame/operation bytes, physical owner capacity, peak and waiting retention. Include compiler, provider and pool resources. Result-return time and final release time differ. Do not require heavyweight per-load traces, universal percentage gates or a large benchmark framework.

## Optimization policy

First obtain a real trace/profile or structural attribution. Prefer redesigns that remove allocations, copies, materialization, checks or task state. Compare one change at a time with matched inputs and identical correctness contract. Report regressions and negative results. No disabled safety checks, weakened assertions or JSON-specific fixture tricks to manufacture speed. Compiler-proven check elimination is encouraged.

For the bounded record loop, compare per-row versus chunked checkpoints and record dispatch CPU, short-task tails and maximum observed cancellation latency. For source-once costs compare backend opt levels only if likely material. Expensive compiler work, caching and fusion need measured justification.

## Result entry template

Append compact entries below. Do not create a generic experiment database.

- Question / observed bottleneck:
- Proposed mechanism and preserved invariants:
- Source commit, platform/toolchain, exact commands and inputs:
- Matched raw result path and summary:
- Correctness tests and cost accounting:
- Decision: keep / revert / further bounded probe, and why:

## Acceptance

Real source parses and becomes machine code. Empty/filter/mixed/overflow cases match a simple semantic oracle; skipped rows do not fail; yields resume without duplicate work. Invalid descriptor/range, permissions, total budget, cancellation and late completion are covered by focused tests. Two actual asynchronous operations run, and a scan failure never dispatches the effectful operation. Waiting on the second operation does not retain the quote owner from the task. Multi-task code reuse does not share mutable state. Checks and limits protect explicit invariants.

Linux ARM64 execution is the target evidence. macOS native runs and cross-builds remain separately labeled. Fuzz/differential checks can be bounded and targeted; no claim that tests prove compiler security. Stop this milestone after the vertical slice, focused independent review and attributable cost report. A negative performance result is an acceptable research outcome, a stub is not.

## Executed vertical slice, 2026-10-13

### Identity and reproduction

Core dependencies are pinned in `Cargo.toml`/`Cargo.lock`. Native correctness builds used rustc 1.93.1. Benchmark builds used rustc 1.96.0, with Cranelift 0.128.3 and Monty commit `56949a4c45f51cedc7a96450782f58a8017e114d` (package version 1.1.0). Published Monty 1.1.0 failed compilation because `&mut Box<[CacheEntry; CAPACITY]>` is not an iterator. The pinned upstream revision already dereferences the box; no local patch or weaker interpreter was substituted.

Local hardware: Apple M4, 16 GiB RAM, macOS 26.4.1. Linux execution: the existing idle two-vCPU, approximately 2 GiB ARM64 Alpine VM, kernel 6.18.52-0-virt, musl. Binaries were cross-built with cargo-zigbuild/Zig 0.15.2 and then **actually executed in that VM**. `bench/results/identity.json` contains source SHA-256 identities, actual toolchains, guest capacity/process observations and commands. Machine code is generated by Cranelift inside the guest process; cross-building the Rust host does not replace native execution.

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo run --release -- fixtures/weighted_scan.py --ir
rustup target add aarch64-unknown-linux-musl
rustup target add --toolchain 1.96.0 aarch64-unknown-linux-musl
python3 tools/run_linux.py <idle-vm-user@host> <ssh-key-path> <known-hosts-path>
python3 tools/run_bench.py <idle-vm-user@host> <ssh-key-path> <known-hosts-path>
python3 tools/summarize.py
```

The reproduction tools do not install guest software or alter system configuration. Rust 1.96, Zig and cargo-zigbuild must already be present. `tools/run_bench.py` without VM arguments runs the macOS part, but the all-platform summarizer expects all four raw files; committed files are available for a local-only replay. No cache service, dependency vendor tree or CI workflow was added.

### Cost accounting

Two source programs: ordinary weighted filtering and a cross-row previous-price/branch variant. Inputs have 0, 100 and 10,000 rows. Every successful receipt is checked against a simple semantic oracle. Each cold/warm/engine/chunk cell has 20 samples. A source-cold invocation always parses, checks, lowers, generates and installs a fresh unit, runs both effects and frees that unit before result return. It is not a process-cold measurement: libraries and compiler process infrastructure are already resident. Warm runs explicitly reuse one compiled unit; this does not assert a real-world reuse rate. The benchmark runtime is shared infrastructure, initialized once before samples. Its construction and final destruction are included in whole-process resource timing, not charged to every invocation.

Four comparisons:

- `native`: producer constructs the same typed QuoteBuffer for that request, transfers its owner through a one-shot provider, executes source-derived native code, then records. No row objects or per-row host callbacks. Two owned Tokio operations are dispatched, even when the underlying provider result is immediately ready.
- `monty`: in-process Rust API, compiled source loop over a list of two-i64 tuples, two direct host suspension/resume boundaries. Producer generation, object/list/tuple construction, VM import, loop and cleanup are inside `total_ns`. No Python wrapper, JSON or IPC.
- Historical `monty_helper`, now named `monty_fixture_helper` in emitted rows and derived summaries: the same QuoteBuffer layer and opaque handle, but the weighted/monotonic business logic is handwritten for these fixtures. Three host boundaries, no per-row callback or arbitrary plan. This is a fixture-specific handwritten performance floor, not the required general helper baseline.
- Historical `host`, now `host_fixture`: checked handwritten fixture aggregation over the same imported buffer, including producer/import/cleanup. This is a performance floor with a different execution/authority contract, not an equivalent sandbox.

`total_ns` is measured directly around each full invocation, including producer construction and cleanup. Compile, conversion and execution columns are attribution only; their medians need not sum to the median total. Warm Monty includes its required runner clone. For native, conversion includes producer/import/provider creation. For the fixture helper, producer/import is inside execution, so `conversion_ns=0` does not mean it is free. `code_bytes=0` for non-native rows is an unmeasured/not-applicable sentinel, not zero bytecode storage. Printing, the correctness oracle and separate retention tracing are outside single-invocation timing. Fixed input settings are batch=17 and limit=700. Delayed operation correctness is tested separately; benchmark ready completions do not represent production I/O distributions.

Four raw CSVs contain 5,976 unique samples, checked and reduced to 336 groups by `tools/summarize.py`. Per platform/opt run: 1,320 successful standalone invocations, 120 successful light tasks, six successful heavy tasks and 48 expected cancellations. Raw CSVs and derived `summary.csv` are committed; no manually transcribed sample aggregation is used.

### Linux results

Source-to-result median microseconds, 10,000 rows, `speed` optimization, native chunk=4,096:

| Source | Native cold | Native warm | Monty cold | Monty warm | Monty+fixture floor cold | Monty+fixture floor warm | Host fixture floor |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Weighted scan | 288.793 | 78.751 | 4749.298 | 4750.652 | 85.959 | 79.230 | 70.146 |
| Stateful branch | 363.043 | 81.397 | 4764.548 | 4776.860 | 83.292 | 81.334 | 71.543 |

The fresh native compile accounts for about 219/292 µs respectively. Native producer/import is about 45 µs on this cold path; execution plus cleanup is about 24/26 µs. Ordinary Monty spends about 2.5 ms constructing/importing objects and about 2.2 ms executing/cleaning up. Avoiding that object layer is the major win. The handwritten fixture floors avoid it too and are approximately 3.4–4.4× faster than cold native here. Warm native is approximately tied with the Monty fixture floor. These historical comparisons alone do not answer the general-helper question; the corrected weighted comparison follows below.

Empty and 100-row fresh-source execution is compile-dominated. At 10,000 rows/chunk=256, weighted cold/warm medians are 304.918/97.334 µs, and stateful medians are 379.399/99.334 µs. Variation across ordered cells includes process warm-up, VM scheduling and CPU frequency; these are small synthetic cells, not confidence intervals or production percentiles.

### Checkpoints and optimization probes

Native scan-only fairness control: one million-row task plus twenty 100-row tasks, pre-imported ready buffers on one current-thread executor. Input construction is deliberately excluded **only from this scheduling control**. Latency clocks start at task submission and stop at completion; the heavy receipt is checked too.

- With per-row checkpoints, heavy scans take approximately 414–415 ms. Light-task maximums in the `speed` run are 164/166 µs for weighted/stateful.
- With 256-row checkpoints, heavy scans take 3.232/3.602 ms; light maximums are 47.458/50.834 µs.
- With 4,096-row checkpoints, heavy scans take 1.689/1.991 ms; light maximums are 85.668/46.792 µs. One batch is insufficient to interpret that weighted tail regression as a stable distribution.
- Instrumented cancellation after a known yield includes dropping the owned million-row buffer. Eight samples per source/chunk; in the `speed` run the largest observed cancellation-to-return time is 82.292 µs. This is not a worst-case response guarantee for arbitrary mid-chunk external cancellation. It does not measure provider-side effect rollback, which is not promised.

Decision: keep chunked reservation/checkpoints; per-row dispatch is a negative result. Leave the conservative default at 256 and expose the already bounded 1–4,096 choice. Do not optimize for one unreplicated tail batch.

`none` lowers 10,000-row cold totals to 260.856/322.710 µs (about 10–11% below `speed`), while warm totals become 81.897/82.813 µs. Large stateful heavy scan also slightly regresses. Retain `speed` as the general default and retain the explicit opt-level experiment hook. Choosing a source-once specialized policy needs more than these two bounded sources. Neither compilation caching nor a new optimizer is justified by this result.

### Ownership, memory and safety evidence

Validated storage is `Arc<[Quote]>` with an exact 16-byte row stride. Its physical row payload is 16N bytes, with no unused vector capacity retained after import. Converting a Vec into the Arc currently allocates/copies the Arc payload: source Vec plus destination payload may transiently occupy about 32N bytes, before allocator/header overhead. This is structural attribution from implementation, not an allocator trace. At one million rows it is approximately 32 MB. Do not hide that copy behind the zero-copy scan claim.

The native scalar/index frame is 280 bytes. Measured Linux Rust layouts: execute future 376 bytes, CodeUnit inline structure 480 bytes, provider inline structure 24 bytes, QuoteBuffer handle 16 bytes, cancellation handle 8 bytes, immediate load/record future bodies 32/24 bytes. These exclude Arc control blocks, channels, Tokio task headers, cancellation Notify storage, compiler heaps and executable mapping granularity. Weighted/stateful machine code is 192/292 bytes with `speed`; retained diagnostic CLIF strings are 2,073/3,053 bytes. Machine-code bytes are not the physical JIT footprint.

`resource` trace lines show task-owned quote/frame payload of 0/0 at the first and second waits. The first provider operation may separately own/produce input. At the second wait, the QuoteBuffer and heap frame have been dropped, and the task has dropped its CodeUnit reference after the proven final native call. Weak-owner tests verify actual release rather than trusting counters. Other callers can keep shared code or independent input owners alive. `TestProvider` deliberately keeps source rows but only a weak reference to returned result storage; the benchmark one-shot provider transfers its typed owner rather than retaining an artificial second copy.

Whole Linux `speed` run: 0.91 s user CPU, 0.86 s system CPU, 1.78 s wall, maximum RSS 40,088 KiB. Linux `none`: 0.90/0.85/1.76 s and 39,904 KiB. These include all engine comparisons, cold/warm controls at the named opt level, common runtime/compiler infrastructure, output and cancellation probes; they cannot attribute per-engine peak RSS or imply a per-task density. macOS maximum RSS is about 54.7/54.5 MB. Raw process counters are in `*-resources.txt`. Separate per-engine allocation counts, JIT mapped bytes and Monty wait-state physical heap retention remain unmeasured; no numeric comparison is asserted for them.

Actual macOS and Linux ARM64 tests: 30 passed on each platform. Cases include empty/filter/mixed, add/subtract/multiply overflow and i64 minimum, short-circuit error order, different source/stateful native IR, chunk resume without duplicate rows, malformed import, independent capabilities, row/buffer limits, provider errors, shared code without mutable-state sharing, cancellation at CPU yields, prompt cancellation during pending I/O, dropped receivers, late completions and pre-record weak-owner release. An overflow never dispatches record. The demo executes real delayed operations and returns receipt=41 with one load and one record; `demo.txt` includes emitted IR.

Cancellation originally only checked after provider completion. Observation: a stuck operation could pin a cancelled task indefinitely. A Notify-backed flag now wakes the receiver independently; detached providers still own their work. Registration precedes flag checking to avoid a lost wakeup. Tests verify prompt task return before a gated provider is released and safe discard of the later result. Releasing the CodeUnit at final native use also avoids pinning its compiler/module resources through the second wait. No safety, resource or authority check was removed.

### Milestone decision and limits

Keep the small typed native compiler as a research slice. The data/ownership boundary and chunked loop work, and generated code changes with source. The two handwritten fixture floors cover these known businesses more cheaply; they do not replace a general helper comparison. Do not expand language scope, implement a cache, infer representative reuse or claim an overall runtime replacement from this campaign. Compiler/lowering, dispatcher and trusted providers remain the TCB; test coverage is not a proof of safe arbitrary untrusted native execution. Independent review of 87e0131 is recorded on the Kanban card; the correction and its remaining approval are described below.

## Generic helper contract correction, 2026-10-13

The independent Luna review of fixed commit `87e0131d97bafce631b98685000b2e7aded3dd18` found no confirmed runtime/lowering blocker. Its report is attached to task `t_eecdee0f` as `independent-review-87e0131.md`. The main experimental gap was the fixture-specific helper. This correction changes only benchmark code and reporting, leaving native semantics, cancellation and Linux acceptance evidence intact.

### Fixed API and coverage

Before the new measurement, Sol fixed one small Quote/v1 primitive, implemented in the bounded Luna lane (`gpt-6-luna/openai-codex`):

`sum_product_where_gt_le(handle, gt_field, gt_bound, le_field, le_bound, left_field, right_field) -> i64`

Field indices 0/1 select price/quantity. The primitive processes rows in order, tests `field > bound` then `field <= bound`, and performs checked multiply then checked accumulation only on selected rows. It accepts no source, IR, expression tree, fixture name or stateful mode. All field/threshold choices are decoded and validated from the actual Monty host call inside timing; the handle is scoped to the single owned buffer. Different field selections and thresholds have correctness tests. This is a finite general operation over the declared schema, not a general query engine. It fuses filtering and aggregation without an intermediate buffer.

Weighted scan calls it with `(handle, 1, 0, 0, price_limit, 0, 1)`. Its compilation, argument construction/validation, producer/import, three direct Monty suspension/resume boundaries, checked helper execution, buffer release before record and final cleanup are inside the total. There is no additional plan compilation. The synchronous helper is bounded here to 10,000 benchmark rows and has no CPU yields/cancellation inside the call. Native has chunk checkpoints and two dispatched Tokio operations. This is not a scheduling, quota or safety-equivalence claim.

The previous-price stateful variant is explicitly **unsupported** by this stateless primitive. Its existing ordinary Monty/native measurements remain valid; the handwritten stateful floor cannot establish its performance against a general helper solution. No arbitrary cross-row logic was rewritten as fixture-specific Rust to fill that cell.

### Bounded rerun and results

`tools/run_generic.py` builds with locked dependencies, records hashes for core/bench sources, fixtures and reproduction scripts, inspects the existing VM, and actually executes the binary on macOS and Linux ARM64. `generic-identity.json` records the parent commit, source hashes, Rust 1.96.0, commands and capacity/process observations. The VM was idle, with approximately 1,763 MiB available and 13 GiB free before running. Only weighted generic-helper, ordinary Monty and native chunk=256/4,096 cold/warm cells were rerun, interleaved per sample. No fairness/cancellation/opt-level campaign was repeated.

```sh
cargo +1.96.0 test --manifest-path bench/Cargo.toml
cargo +1.96.0 clippy --manifest-path bench/Cargo.toml --all-targets -- -D warnings
python3 tools/run_generic.py <idle-vm-user@host> <ssh-key-path> <known-hosts-path>
python3 tools/summarize_generic.py
python3 tools/summarize.py
cargo test
```

Local-only reproduction omits the three VM arguments. Six benchmark tests passed, including field/threshold permutations, empty/filtered input, skipped overflow, multiply/add overflow and actual Monty helper call/receipt parity. Core macOS tests remained 30/30. The toolchain's missing Clippy component was installed specifically for Rust 1.96 without changing the global default; its pre-existing manual-async lint in the immediate record provider was corrected without changing its behavior.

`generic-macos.csv` and `generic-linux.csv` each contain 480 unique successful invocations, covering 3 row counts × 2 modes × 4 engines/chunks × 20 samples. `summarize_generic.py` checks every receipt against its independently computed row oracle and writes 48 summary groups. Historical raw files and identity are unchanged; `summary.csv` now labels their old `monty_helper`/`host` columns as fixture floors.

Full source-to-result medians, 10,000 weighted rows, microseconds:

| Platform | Generic helper cold/warm | Ordinary Monty cold/warm | Native 256 cold/warm | Native 4,096 cold/warm |
| --- | ---: | ---: | ---: | ---: |
| macOS M4 | 25.459 / 18.625 | 2759.271 / 2590.333 | 140.500 / 22.250 | 136.667 / 20.250 |
| Linux ARM64 VM | 83.334 / 71.772 | 5033.777 / 4996.402 | 312.168 / 82.147 | 279.334 / 67.355 |

On Linux the helper's compile/clone attribution is 13.750/0.750 µs, with producer/import/execute/cleanup inside its 69.396/70.501 µs execution attribution. Native 4,096 cold compilation is 214.189 µs, producer/import 43.542 µs, and execution/cleanup 22.062 µs. Medians of attribution columns do not sum to the median total. Whole-process CPU/RSS are in `generic-*-resources.txt`; they still cannot attribute per-engine allocations or density. This rerun does not fill the previously unmeasured memory categories.

Decision: the parameterized finite helper beats source-cold native by about 3.35× on the Linux weighted case. Warm native and helper are near each other, with the direction dependent on chunk/platform in these small samples. The result reinforces the value of avoiding object materialization for expressible aggregations; it does not resolve arbitrary/stateful user-code value. Keep the bounded native slice, no cache, syntax expansion, new runtime or unbounded campaign. Submit this baseline correction for the original same-card approval; do not repeat the completed global implementation review.

## B performance-v2 checkpoint, 2026-10-14

[Bounded M4 A/B evidence and commands](b-performance-v2.md): retain opt-in CLIF and Cranelift pass timing, removing ordinary compilation's diagnostic string and timer work. Four alternating rounds against fixed `564381d` show 0-row cold source-to-result medians 113.146 → 100.229 us and 10,000-row/chunk=4096 medians 134.250 → 121.833 us. Machine code remains 192 bytes; warm-path differences are not claimed as gains. Default and timing-enabled core validation each pass 31 tests. Formal Linux and density comparisons remain C work.

An unmerged scratch `Arc<Box<[Quote]>>` owner preserves nonempty payload pointers and avoids Vec-to-Arc copying. One-million-row warm-code native totals including generation/import/cleanup have pooled medians 1.423 → 1.057 ms, with a slower first candidate round retained. Counting-allocator peak requested bytes fall 32,000,016 → 16,000,032; these are not RSS. With only Weak observers, current `Arc<[Quote]>` still retains 16,000,016 requested bytes versus 32 in the candidate. Failed Weak upgrade tests establish strong-owner release, not allocation release. The public `Weak<[Quote]>` observer type change needs an explicit decision before merging. The report includes the narrow patch, raw receipts, exact probe source and the excluded stale-build failure; no runtime instrumentation or new safety checks were added.

## C integrated Linux cost/density checkpoint, 2026-10-14

[Formal report and figures](performance-v2.md), [benchmark contract](benchmark-v2.md), [next bounded candidates](next-steps.md). A `37bd766` and B `8ef381a` were signature-checked and integrated; owner candidate remains unmerged. Formal measured source/harness `065c7cb` versus fixed runtime `564381d`, identical bench dependencies, actual idle 2-vCPU ARM64 VM, Rust 1.96.0/Zig 0.15.2. Three alternating process rounds complete 234/234 commands; independent Python receipt/accounting verification covers all 1,008 latency and 5,472 finite-load observations. Core passes 31 tests on macOS and actual Linux; bench 10, Python 6, Clippy with warnings denied, integrated macOS smoke 17/17.

Weighted 10,000-row cold total falls 350.17 -> 289.50 us, ordinary Monty 3,712.72 us; 100-row native remains slower than Monty (209.29 vs 117.63 us). Source-once compile remains about 198 us weighted / 286 us stateful at 10k. Zero-row full-task first-wait requested-layout slope is about 5.36 KiB native / 48.51 KiB Monty, not RSS or frame size. At record wait for 8 x 10k rows, native sampled RSS is 4.789 MiB versus Monty 17.129 MiB; explicit Monty quotes=[] still 16.551 MiB because VM capacity survives logical row release. No Weak observer or allocator trim in density measurements.

Prepared burst is distinct from the added finite cold-arrival control (64 mixed requests, one/ms, admission 8). With 1-ms sleeps/op, cold short p99 is 7.983 ms native / 57.451 ms Monty, a bounded observation rather than service capacity/SLO. Monty rejects del, so clear-list rebinding is separately labeled and tested. Native 10B Vec control reports 34 requested storage bytes but has no language/task execution: it is not a string result. Linux peak instrumentation was corrected twice before the formal campaign: inherited getrusage HWM, then slightly decreasing raw VmHWM; raw kernel values and cumulative observed max RSS/HWM are now separate. Retired pilots stay ignored, not pooled. Setup/verifier assumptions and failures are explicitly recorded in the report. No CI, paid resources, global configuration, new VM or other project changed. D is pre-created by the operator and releases after C, rather than extending C indefinitely.

## D script entry and bounded performance iteration, 2026-10-14

User priority changed to a usable top-level Python script with inferred scalar
types, ordinary sequential Tool calls and explicit scalar print. Implementation
`c11c7d1` replaces the fixed scan shell with one semantic IR and one native CFG.
Resume PCs exist only at actual Tool waits and bounded row-chunk yields. Source
names bind to slots at compilation. The legacy benchmark adapter remains.
Expression statements are evaluated/discarded, including bool-valued final
expressions and unused declared Tool returns. Loop-body locals can be introduced
where assigned; potentially empty loops do not establish initialization afterward.
Scalar `print(tools.record_total(...))` lowers to the existing call and print
operations. `await print` is rejected. No new runtime dispatch or second IR was
needed for those source forms.

Scalar, scan-without-record and scan/record/receipt-print paths execute real
generated machine code. Tests include renamed inputs/rows, branches with Tool
resumption, sequential loops, loop-local arithmetic, print within loops, multiple
buffers, aggregate retained-buffer bounds, cumulative fuel/call limits, permissions,
earlier output on failure, cancellation and late completion. 45 core tests pass
on macOS and the actual Linux ARM64 VM; bench 12 and Python 6 pass. Core and new
benchmark Clippy pass with warnings denied. Frontend lanes ran on metadata-verified
`gpt-6-luna/openai-codex`. The separate Codex CLI rejected that exact model for its
ChatGPT account; no substitute model was used. Sol owns integration and conclusions.

### Four retained replacements

All A/B runs are serial, alternating per independent process round on the same
machine, with the same source/producer/ABI/lifecycle within each pair. Raw samples
include initialization. Tables summarize warmed-process requests, with first
request distributions retained in `summary.json`. Unless specified otherwise,
discard samples 0–9 for summaries, three rounds and 50 retained requests per
round. Compiler scratch reuse keeps fresh source parsing and compilation.

| Candidate | Linux operation | Wall before / after | Material cost or evidence |
| --- | --- | ---: | --- |
| `fa64b32` cleared compiler scratch | printing weighted script, cold 100 rows | 390.23 / 338.77 us | about 33 KiB retained per compiling thread for this source; steady allocation calls 793 / 758, reallocations 239 / 170 |
| `c4a503a` live scalar writeback | same, cold 100 / 10k rows | 356.89 / 331.73 us; 443.91 / 410.79 us | Linux machine code 648 / 572 bytes; compile-time backward liveness, no runtime guards; Host inputs already resident and immutable |
| `c0afa8c` bounded geometric stdout | 4,096 scalar prints, cold / prepared | 467.74 / 378.84 us; 223.09 / 148.13 us | 4,095 / 12 reallocations; cumulative requested allocation 18.78 / 0.165 MB, not measured bytes copied; output capacity 9,103 / 16,384 bytes |
| `af06948` boxed Quote payload | 1M mixed rows, cold / prepared | 5.479 / 4.900 ms; 5.182 / 4.501 ms | input generation/freeze/cleanup included; requested peak 32.05 / 16.05 MB; extra small owner allocation/indirection |

Scratch mixed 64-request/1-ms-wait CPU medians fall 34.84 -> 32.23 ms and pooled
short p99 32.24 -> 28.82 ms on Linux. Live-writeback CPU falls 34.13 -> 31.84 ms,
short p99 31.17 -> 28.30 ms. These finite all-admitted burst observations have no
fixed-arrival/admission model and are distinct from C's arrival experiment.
M4 scratch gains are near noise; its pooled timer tail rose in one round. M4 live
writeback cold wall improves 161.21 -> 146.46 us at 100 rows, and 186.50 -> 170.69 us
at 10k. Its mixed CPU improves, while timer p99 is noisy. No cross-platform
universal tail or SLA claim follows. Linux is the measured deployment target.

### Ownership/API decision and physical control

The operator's latest unattended authorization explicitly permits measured
engineering/API decisions. The previous owner approval gate was superseded.
Frozen storage now uses `Arc<Box<[Quote]>>`; `QuoteWeak = Weak<Box<[Quote]>>` names
the public observer type returned by `downgrade` and `TestProvider::last_rows`.
Typed observers/tests are migrated. The exact-capacity freeze preserves its
payload pointer; spare Vec capacity may shrink. Byte import still decodes/copies
into aligned typed storage. The old error enum adds `Input` for the new sealed
scalar argument signature. External exhaustive error matches and explicit old
Weak types need migration. Legacy compile/execute/Provider signatures are intact.
The internal public IR fields change with the approved single-IR migration.

Owner was first tested in an isolated `c0afa8c + owner-candidate.patch` worktree.
With no Weak observer, 1M-row load wait is about 21.23 MB RSS in both arms, then
record wait about 5.22 MB. Requested peak halves; observed kernel/RSS high water
is about 37.11 / 21.23 MB. This describes observations, not exact physical worst
case. A separate explicit-Weak real-task control shows old storage retaining
16.05 MB requested / 21.21 MB RSS at record wait, versus about 0.046 MB / 5.21 MB
with the boxed owner. Dropping the old Weak releases that payload. These controls
separate strong-owner release, allocation release and physical residency.

### Generalization cost and strong baseline

The final runtime snapshot is `1f8b7d7`. Ordinary Monty executes the full adapted
script, including print and actual asynchronous Tool suspension. The adapter
changes declared Tool syntax to explicit await and field reads to tuple indices;
no user arithmetic/filter/loop is replaced with Rust. Both arms use independent
equivalent typed producers. Speed uses System without allocation diagnostics;
memory uses a separate counting build. Source clone/compile, argument and provider
construction, freezing/conversion, native/VM execution, real waits, code/task/
payload cleanup are timed. Task results/stdout remain live for correctness
checking outside timing, so output delivery/destruction is not included. Prepared
code compilation is outside its explicitly prepared boundary.

`integrated-linux.jsonl`: 48 completed serial commands, 1,920 successful request
observations, three rounds, 30 retained requests per cell/round after sample 9.

| Operation | Native cold / prepared wall us | Ordinary Monty cold / prepared wall us |
| --- | ---: | ---: |
| scalar compute/print, no Tool | 190.00 / 2.00 | 56.63 / 39.85 |
| weighted scan/record/print, 100 rows | 640.99 / 7.50 | 247.92 / 221.00 |
| weighted scan/record/print, 10k rows | 466.03 / 86.02 | 3,799.23 / 3,757.16 |

Process medians vary substantially (native 10k cold rounds 696.38, 418.90,
466.03 us), even on the idle VM. No unmeasured cause is assigned to that variance.
Small cold scripts still lose to Monty; compilation is the largest remaining
cost. Independent median attribution columns are not an additive decomposition.
Equivalent legacy native source returns an integer rather than printing it:
100-row cold/prepared 370.77 / 8.71 us, 10k 511.76 / 124.23 us. Different code/
output boundaries explain why this is a migration reference, not a zero-cost
syntax equivalence claim.

The unchanged `bench-v2` harness also compares C's runtime and D on the exact old
source/producer/timer boundary. At 100 rows cold wall grows 380.33 -> 525.02 us,
prepared falls 8.67 -> 6.73 us. At 10k cold grows 485.56 -> 599.46 us, prepared
falls 119.71 -> 94.73 us. Machine code grows 192 -> 448 bytes, frame 280 -> 320
bytes. The new general native continuations cost compilation and code footprint;
the source contract was explicitly prioritized. C's 39-cell campaign was not rerun.

Separate gated 64-task unique-code memory probes use concrete per-engine futures,
so an inactive Monty branch does not inflate native task storage. Each task has
an independent payload. At zero rows native first/record wait requested storage
is 573.6 / 571.6 kB, Monty 3,249.3 / 3,250.3 kB, including code, providers, gates,
handles, sources and compiler scratch. At 1k rows each: first native 1,597.6 kB,
record 572.5 kB; Monty first 4,273.3 kB, record 9,387.7 kB. Native sampled RSS at
record is 6.23 MB versus 16.99 MB. Code needed for post-record print is retained;
terminal legacy record uses only 154.4 kB requested at zero rows. These are full
probe states, not frame sizes or a measured density slope.

### Negative results and convergence

Initial 100-compile Linux stage profile (temporary instrumentation, removed)
has medians parse 34.17, prepare 1.31, lowering 16.81, backend 299.82 and finalize
2.46 us after sample 9. ISA setup is too small to justify a cache. Compiler
scratch targets allocations in lowering/backend instead.

Two final bounded alternatives were reverted: `opt_level=none` gives inconsistent
cold benefit, worsens pooled 10k cold wall 492.75 -> 542.42 us and grows weighted
code 572 -> 600 bytes; 1M prepared is neutral/slightly worse. Incremental active-
owner/byte accounting removes repeated owner scans, but 10k prepared wall
121.75 -> 152.46 us regresses and rounds are unstable. Its 1M improvement is too
small/inconsistent to repay extra retained state/invariants. Neither code path,
flag or production instrumentation remains.

No evidence supports another simple net-value candidate in this lane. Remaining
compiler/admission changes need a new cost hypothesis and workload; bytes/string
values need encoding/ownership/Tool-ABI choices before implementation. Private
stdout is not a first-class string value. Stop at this verified slice, not at a
claim that the whole system is optimal. No global configuration, paid resource,
new VM, Docker, CI or unrelated project changed.

Reproduction and identity: `bench/results/d-script/identity.json`, raw JSONL,
`summary.json`, and `linux-tests-integrated.txt`. Speed/memory executables are
separate. Target is actual Alpine Linux 6.18.52 ARM64, existing 2-vCPU/2-GiB VM on
M4; Rust 1.96.0, Zig 0.15.2, unchanged pinned Monty. Core macOS uses Rust 1.93.1.

```sh
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo +1.96.0 test --locked --manifest-path bench/Cargo.toml
cargo +1.96.0 zigbuild --release --locked --tests --target aarch64-unknown-linux-musl
cargo +1.96.0 zigbuild --release --locked --manifest-path bench/Cargo.toml --target aarch64-unknown-linux-musl --bin script-bench --bin bench-v2
# For memory: add --features allocation-diagnostics; do not reuse its timing.
python3 tools/run_d_scratch.py KEY KNOWN_HOSTS HOST
# D_LABEL=spill D_BASE=... D_CANDIDATE=... uses the same bounded runner.
python3 tools/run_d_owner.py KEY KNOWN_HOSTS HOST
D_OUT=integrated-linux.jsonl D_BIN=./d-integrated-script-speed-linux python3 tools/run_d_final.py KEY KNOWN_HOSTS HOST
python3 tools/summarize_d.py
```

Failures were explicit and corrected: an unfilled native dispatch block in early
integration; an initial sleep-based lifetime assertion replaced with a phase-based
wait; Clippy-only observer alias/test placement; one wrong SSH host argument with
zero observations; and tool stdout truncation/redaction while collecting v2 rows.
The final v2 raw data was saved remotely and copied as files before parsing.
Exploratory union-future memory and non-reset peak output are not pooled into
published memory results. Earlier stages and all first requests remain separately
labeled. The final fixed-snapshot focused review of `255e4eb` found no confirmed blocker.
The reviewer independently reran 45 core and benchmark tests. Actual
`gpt-6-luna/openai-codex` identity is verified by the separate Hermes CLI usage
record. Native delegation was capped at two already-consumed frontend lanes;
the review did not change that budget or any configuration. Review text and
identity are archived as `focused-review.txt` and `focused-review-identity.json`.
No per-candidate review loop was added.

## E cold-compiler diagnosis (2026-10-14)

E profiles the unchanged D compiler/backend, without implementing an optimization.
The bounded real-Linux campaign starts from `4abb2a1`: six source shapes, three
rounds, interleaved ordinary/diagnostic release builds, 2196 compile observations
plus 12 untimed pre/post CLIF inventories. First process compilations and scratch
transition samples are separated from fresh-source steady-process compilations.
Both default and opt-in macOS configurations pass 45 core tests and clippy;
36 Linux driver processes and six inventory oracles execute actual generated code.

Ordinary compile medians are 227.46 us scalar, 330.12 us weighted and 459.25 us
two-scan/conditional-Tool; empty-source control is 121.63 us but still includes our
generic frame/PC entry. Diagnostic `define_function` accounts for 82.1--85.8% of
compile wall. Weighted exclusive register allocation is 87.57 us and verification
75.74 us. Each compile performs five verifier calls, all retained. Setup/finalize
are only a few us. This favors a bounded same-backend predicate-CFG experiment
before a backend replacement, while not identifying a universal fixed-cost floor.
Profiling perturbation is material: weighted round-matched overhead is +4.9--8.2%,
and other cases vary more widely. No uninstrumented pass times are inferred.

Historical single-pass evidence lowers allocator priority: four alternating
ablation rounds at `564381d`, old IR/macOS M4, 10k weighted rows/chunk4096 and
ready providers, with diagnostics omitted and repeated verifier retained, give
source-cold 117.604 -> 134.7705 us and prepared 20.2295 -> 42.792 us.
Source: `/Users/yuzhe/.hermes/cache/scratch/minipy-hotspots/FINDINGS.md:3,10,14--21,33`.
These are old end-to-end negative results, not E's Linux measurements. E did not
implement any candidate; the current general CFG/Linux allocator combination
remains untested. Predicate lowering ranks first, no-suspension specialization
second, and allocator retest only third under those explicit new conditions.
The regalloc bucket is not an expected net saving; runtime regressions must count.

See [compiler-profile.md](compiler-profile.md) for the phase/pass boundaries,
nested-timer accounting, IR inventories, three ranked candidates and their history,
commands and limitations. Exact hashes, gzip raw and recomputed summary are in
`bench/results/e-compiler-profile/`; both runner and summarizer use Python stdlib.
No default profiling/IR traversal/formatting, source cache, new backend, verifier
suppression, runtime/string/scheduler change or Monty rerun was introduced.

## F predicate-oriented conditional lowering (2026-10-14)

Keep the local `Stmt::If` replacement over `51e83bd`: short-circuit predicates
branch directly; bool assignment/print still materialize their ordinary values.
Weighted/nested CLIF blocks fall 25 -> 23 / 39 -> 31, code bytes 572 -> 548 /
704 -> 636; total SSA parameters do not uniformly decrease. No safety, effect,
resource, allocator, verifier, runtime or language contract changed.

The initial Linux 6060 samples + 12 inventories exposed scalar control noise up
to ~24%. Exactly one longer follow-up used the same binaries, 36036 retained
samples, final 500/process for steady-process summaries. Cold source-to-result
wall medians: scalar 130.797 -> 130.881 us, weighted/10k 324.437 -> 287.923 us,
nested/10k 466.784 -> 414.577 us. Cold CPU tracks wall; all three rounds improve
both scan shapes while the scalar cold control stays within 0.7%. Nested prepared
round zero regresses 6.7%, later rounds improve ~21/22%; no universal execution
speedup claimed. Both macOS configurations and actual Linux pass 48 core tests;
clippy passes. The VM is restored off and released to the separate G lane.

See [compiler-profile.md](compiler-profile.md#f-predicate-lowering-experiment-2026-10-14)
for boundaries, all round changes, reproduction and limitations. Small raw,
source/binary identity, CLIF, patch and test log are in `bench/results/f-predicate/`.
No new Monty/C/D matrix, second optimization, cache, backend, CI or PR. Stop at the
focused same-card review handoff.

## G: explicit lightweight typed-IR interpreter

G is isolated at `51e83bd`, with native lowering unchanged and no automatic backend switch.
The resident-service library path shares frontend/Quote/Provider/cancellation/output
contracts and replaces native compilation with bounded statement flattening and
typed expression interpretation. Actual same-round ARM64 Linux cold medians:
scalar native/interpreter/Monty 167.123/27.026/38.659 us; weighted 100 rows
262.770/50.673/133.056 us; weighted 10k 328.899/328.696/3940.481 us.
Weighted 10k is neutral cold and slower prepared (interpreter 300.340 vs native
79.716 us), so this remains an explicit experimental small-task option, not a
native replacement. No F measurements enter G's comparison.

18,090 primary raw samples, 4824 strong Monty list-release control samples and
40 separately instrumented gate-memory rows are retained. Initial 52 core tests
ran on actual Linux and macOS. Review reproduced a flaky chunk-cancellation wait
in `ac08ca1`; a test-only gated-load/first-chunk handshake repair now passes all
54 macOS release tests, plus 50 focused and 10 interpreter-suite repetitions.
The repaired tests and final two error/receiver suites have not been rerun on
Linux; production code and performance evidence are unchanged. Both VM sessions
restored off with vfkit exit 0. Detailed boundaries, density caveats, keep decision,
commands and source/binary identities: [interpreter-experiment.md](interpreter-experiment.md),
`bench/results/g-interpreter/`. No main integration or hot-loop JIT was performed.

## H: explicit interpreter integration and average CPU cost (2026-10-14)

Signed merge `b3a12fd` integrates reviewed G `9b28716` into F `8cb905b`;
native predicate lowering is preserved byte-for-byte. Runtime/IR interpreter
production source remains reviewed G. Caller-selected library interpretation
shares the native semantic/authority/lifecycle contract. CLI remains native;
there is no automatic backend policy, threshold, hot-loop JIT, OSR or source cache.

Actual Linux ARM64 campaign records 72,600 speed rows and 40 gated memory
snapshots. Two controlled 40-task mixes have 60%/20% scalar work, 100/10k/100k
scans, planned 500-us arrivals, 100-ms finite window budget, real ready/1-ms Tool
providers, and explicitly labeled prepared reuse. Cumulative CPU/task, arithmetic
latency and short-task p95/p99 are recomputed from raw, not averaged shape medians.
Overloaded windows remain in raw and summaries; these finite draining batches
do not establish sustained fixed-rate service capacity.

Light cold interpreter CPU/task is 578.94/587.95 us (ready/delayed), versus native
698.88/730.55 us and ordinary Monty 1855.33/1963.86 us. Scan-heavy cold native and
interpreter CPU are roughly equal. Prepared native is cheaper in both mixes;
100k stateful native cold CPU 843.88 us versus interpreted 2629.60 us. Interpreter
reduces measured short-task cold tails. Record-wait requested increments are
86,134/124,264 bytes (light/scan) interpreter versus native 181,436/301,874,
ordinary Monty 27.37/54.48 MB and the secondary list-clearing Monty 26.21/52.14 MB.
RSS/PSS and sampled high-water are retained because JIT mappings are not all
counted by requested allocations; no density capacity extrapolation is made.

The single candidate removed retained/dispatched Jump after an if with empty
else. Mixed CPU changes -2.29/-0.53/-0.85% (light ready/delayed/prepared) and
-1.61/+0.29/+1.27% (scan) are not stable net value; small cold scan cells regress.
Revert the candidate, preserve the exact patch/raw, retain its differential test,
and stop without adding another optimization. Final macOS default/diagnostic and
actual Linux default each pass 58 tests, including repaired G cancellation tests.
The initially off VM was restored off after execution. Full boundaries,
commands, identities and limitations: [average-cost-experiment.md](average-cost-experiment.md)
and `bench/results/h-average/`. One final focused fixed-commit review remains.

## I execution reference and bounded strategy qualification (2026-10-14)

Added a feature-gated Rust reference for scalar/weighted/cross-row stateful fixed
fixtures, using the same Provider, detached Tool protocol, bounded Output,
authorization, cumulative rows/chunk yield and cancellation machinery. It is
execution-only and does not claim source compilation. Added three-way semantics
and gated owner/receiver-drop/late-completion tests. No source-string fast path.

Existing ARM64 Linux VM, exclusively started from off (2 CPU / 2048 MiB), was
actually used. H shapes/weights/planned arrivals/windows remain unchanged. Same
round four-engine baseline has 72,600 raw requests; two independently qualified
candidates each have 36,300. Speed has no diagnostic counters/allocator. Baseline
overload 199 windows is retained; both candidate campaigns have zero. Small
standalone cells contain substantial scheduling/frequency variability, so their
nominal ranking is not generalized. At 100k stateful prepared CPU: fixed reference
504.09 us, native 581.67 us, IR 2542.49 us, Monty 35301.68 us. Native large execution
is close to the reference's host/protocol floor; cold tiny native remains compile
dominated. IR event counts and auxiliary macOS sampling locate recursive eval
work without pretending to provide Linux per-subphase cycle shares.

Candidate A bypasses recursive eval for leaf operands, removing 520172/427064
eval crossings for 100k weighted/stateful diagnostic inputs. Mixed CPU improves
4--14% in all six cells and every paired round. Ready short p99 rises in all three
rounds (pooled 2561.5 -> 2990.8 us light, 3468.0 -> 4095.9 us scan); other tails
and tiny cells are noisy. This supports conservative reversion, not a claim of
stable causal degradation or a requirement that every cell improve. Preserve
the CPU gain and tail uncertainty. Candidate B eliminates PC load/
Switch for exactly one actual resume state, scalar code 264 -> 236 B in auxiliary
Mac CLIF inventory. Mixed CPU is not consistently better and prepared/delayed
cells regress. Revert. No third attempt, hidden alternative or new optimizer.

Reference record-wait requested increments 53686/69608 B (light/scan) compare
with IR 86134/124264 B and native 181436/301874 B. Ordinary and list-cleared Monty
remain measured; RSS/PSS/high-water complement allocations. No maximum service
task count or hard process-memory quota is inferred.

Final core macOS all-feature and actual Linux fixture-reference each pass 61
tests; core strict all-target/all-feature clippy and formatting pass. Benchmark
clippy passes with only the two existing intentional large-enum/cfg-assert lints
allowed. Rejected patches apply cleanly to the final tree. Linux test binaries
were SHA256 read back and executed serially, not merely cross-built. Guest was
restored off after validation. Full cost/architecture recommendation, limits,
commands and identities: [stage-strategy-experiment.md](stage-strategy-experiment.md)
and `bench/results/i-stage/`. Chinese stage conclusion:
[stage-performance-conclusion.md](stage-performance-conclusion.md).

The sole independent read-only review examined signed d8f8d99 through native
Hermes delegation, with actual returned model gpt-6-luna / openai-codex. The
earlier Codex CLI request for that model failed HTTP 400 and performed no review;
it is not counted as a completed review. Native delegation succeeded without a
global profile change or model substitution. No blocking finding. The reviewer
reran macOS all-feature release tests; the parent independently reproduced all
three raw summaries exactly and verified both logged 61-test totals and ready
p99 round values. Final changes only clarify those conclusions and add the
requested Chinese entry; no further runtime candidate or campaign.

## J: bounded hotspot iteration (2026-10-14)

Revisited I's leaf-operand inline with genuine AB/BA/BA/AB ordering; the earlier
two-configuration campaign loop inadvertently always ran baseline then candidate.
Actual exclusive Linux ARM64 execution retained 58,128 raw rows across a chunk
sensitivity experiment, one row-present-check candidate and a final mixed regression.
Keep leaf inline under the user-authorized average CPU objective: final six mixed
CPU cells improve 6.66--20.48%, all 24 pair directions improve. Disclose the scan
delayed short p99 +9.39% (all four pairs worse), scan warm pooled +11.92%, and
nonuniform tiny/weighted standalone values; do not claim a tail optimization.
The row-present-check removal has no stable net gain and is reverted. Default
chunk, checked arithmetic, bounds, permissions, cancellation and cumulative fuel
are unchanged. No mixed scan JIT, cache, scheduler or new language contract.

macOS all-feature and actual Linux fixture-reference each pass 61 tests. Linux
test executable SHA256 values were read back before serial execution. Core strict
clippy and both fmt checks pass; bench Rust 1.96 clippy retains only its two prior
intentional lint exceptions. Diagnostic counts are separate from speed; final
rebuilt speed binary exactly matches the measured leaf SHA256. The initially-off
VM was restored off (vfkit exit 0). Full trade-offs, commands, patches, raw and
identities: [hotspot-iterations.md](hotspot-iterations.md), `bench/results/j-hotspots/`.

## K: final low-cost hotspot round (2026-10-14)

Revert the sole candidate, gating interpreter Next's scan-header lookup on the
existing chunk boundary. An uninstrumented macOS ARM64 sample still locates
eval/dispatch work; release assembly confirms the old header bounds/tag lookup
precedes the chunk branch. Separate actual Linux diagnostic counts drop those
lookups from 100000 to 391, with all other work counts and stdout unchanged.
This work elimination does not translate into stable source-cold average gains.

Four matched AB/BA/BA/AB Linux pairs retain 20,560 raw requests. Cold light CPU
regresses 1.76--2.06%; cold scan improves 1.69--2.66%, with mixed pair directions.
Scan warm improves 6.65%, while 100k stateful cold/warm differ by only -1.09/-0.65%
with mixed directions. Delayed scan short p99 rises in all four pairs, pooled
8617.85 -> 9036.24 us. All raw overload windows: zero. Keep noisy shape/block
values, not a selected best-cell conclusion. No second candidate or final
four-engine campaign in this round.

Production src is identical to b76be2d; rebuilt speed is byte-identical to the
measured baseline and J leaf. Retain one new chunk-boundary/budget differential
test, the rejected patch, raw, hashes, commands and summary. Candidate interpreter
tests pass on macOS and actual Linux; final macOS all-feature and actual Linux
fixture-reference each pass 62 tests. Core strict clippy and both fmt checks pass.
The exclusively started existing VM is restored off. Full accounting and stopping
decision: [final-hotspot-round.md](final-hotspot-round.md),
`bench/results/k-final-hotspot/`. L owns the final frozen-version benchmark/plots.

## L: frozen end-to-end benchmark and figures (2026-10-14)

Freeze production `8a293ec54c6e7aed72e23a5c1557a3bdc3b44f60`; `src/` and dependency
locks remain unchanged. Actual serial Linux ARM64 execution covers 60,696 primary
requests (108 speed cells), 360 primary allocator/physical snapshots (120 cells),
CPython 3.14.7 spawn/resident/prepared/prewarm+fork, and Pyodide 314.0.7 on
task-local Node 24.18.1. No affinity change, Docker, CI or global package install.

Add 25,350 common-body observations: MiniPy/Monty read identical body bytes;
CPython/Pyodide add only an explicit async-function wrapper and indentation,
asserting an exact body round trip. Data adapters and their costs are explicit.
At the pinned Monty revision the initial namedtuple Tools adapter fails its
attribute lookup smoke; retain the failure and use existing module/eager
HostClass interfaces without rewriting the program algorithm.

A real handwritten checked AArch64 kernel retains multiply/add overflow,
predicate order and chunk boundaries. Keep the initial 108B version and one
160B mode-hoisting refinement. The latter roughly matches the optimized Rust
weighted kernel, while stateful remains slower; no physical-optimum claim.
Remove an unnecessary out-of-timer retained fixture from full-path harness;
final full values use 7,272 separately paired observations, kernel-only values
use their own valid cohort. All prior raw is retained, not pooled as a winner.

Record a bounded 12,012-row longer warm-scalar recheck separately because tiny
primary values vary by round. Independent process bootstrap probes include
actual CPython/Node startup, import/init, IPC where applicable and reap. Treat
stateful100k as predicate-dominated: only 78 selected rows execute mul/add for
the actual generated input, not an arithmetic-heavy workload inferred from N.

Report, raw plans, identity, hashes, commands and two SVG/PNG figures:
[benchmark-overview.md](benchmark-overview.md), `bench/results/l-final/`,
`tools/final_{summary,workload,plot,report}.py`. Waiting process-tree PSS is
separate from compiler/execution cost, source and isolation equivalence.
AF's subsequent source-once open-loop capacity campaign is documented in
[AF capacity report](af-capacity-benchmark.md). It retains 90/90 confirmations,
870,063 offered requests with exact raw reduction and separate planned-arrival
latency, rejection/debt accounting and finite-window capacity brackets.
Production runtime and Guest remain frozen; no new website changes.

Five reduction/wrapper tests and real Linux assembly/Host tests pass; frozen
native/IR/reference cancellation, late completion, owner release and effect
errors pass on macOS and actual Linux. Final focused review/recomputation and
VM-off verification are delivered with the final evidence handoff.

## N compact interpreter and approved corrections (2026-10-14)

After L acceptance, independently committed the bounded definite-assignment
bitset (`a4d68c1`) and existing-output statement emission (`81d5df0`). Replaced
recursive interpreter expressions with one sealed computation tape during the
existing flatten traversal, preserving static leaf operands, native predicates,
checked arithmetic, short-circuit/error/effect order, owner/fuel/cancellation and
the original Scan/Next header behavior. No retained Expr copy, global optimizer,
JIT, cache, service worker, permit or JSON support. Scratch is synchronous-only.

Same-round Linux ARM64 baseline 8395039 versus integrated candidate: 20,560 speed
raw, four AB/BA/BA/AB pairs, six mixed CPU cells all improve in every pair. Cold
mixed CPU improves 23.40–46.04%; 100k stateful cold/warm improves 48.29/53.09%.
Weighted 100 cold regresses 15.82%, scalar warm adds 2.98 us, and some short p99
cells regress. Preserve all raw/rounds and explicit reuse boundaries. 96 waiting
snapshots show unique-unit overhead down 232/320 bytes per standalone task;
shared-unit savings are proportionately smaller, physical PSS nearly unchanged.
Preparation wall and two separately attributed macOS frontend stages (11,616
raw) are included, without claiming uniform small-input or native compile gains.

Confirmed/documented that chunk is per scan, not a task-wide CPU slice, and
cancelled detached Providers lack global admission quotas. Both are existing
reachable service limits; scheduling semantics are unchanged. New regression
tests cover 0/32 initialized slots, self-reference, Boolean consumers, scratch
after Tool/chunk boundaries, and consecutive exact-chunk scans without yield.
macOS all-feature and actual Linux fixture-reference each pass 66 tests; strict
clippy/core+bench fmt pass. Runner memory-parameter assertion and stale diagnostic
build reuse were corrected transparently; speed raw was not rerun and forced
speed/allocator rebuilds match measured binaries byte-for-byte.

Full Chinese result, costs, limits and reproduction:
[compact-interpreter-experiment.md](compact-interpreter-experiment.md),
`bench/results/n-compact/`, `tools/compact_{ab,summary,verify}.py`.
L figures remain a frozen older-version cohort, not evidence about N. Stop after
one focused fixed-snapshot review and signed publication, without new follow-up
cards or implementing proposals 2/3.

Focused read-only review fixed signed `6c150ea`, actual delegated model
gpt-6-luna/openai-codex: no blocker, independent summary exactly matches and
14 interpreter tests pass. Only raw log/section whitespace is non-blocking;
source/docs/tools scoped diff check passes and evidence formatting is preserved.
After review only documentation/handoff records changed. Guest poweroff verified
by vfkit exit=0, stopped restful endpoint and zero matching process. See
`bench/results/n-compact/focused-review.json`.

## S frozen compact-runtime WSL campaign (2026-10-14)

Runtime and original request harness are frozen at `969e679`; no `src/`,
fixture or demo changes. WSL `na@DESKTOP-UISID0V` is actually reachable and idle,
Linux x86_64 on an i9-7920X with 24 visible CPUs and about 23 GiB RAM. Build trees,
downloads and raw stay in `/home/na/benchmarks/minipy-s`; only small evidence
returns to Mac. The task-local plan is `bench/results/s-wsl/plan.json`.

Completed 690 planned commands: 660 supported completions, 20 observed old
Pysolate ABI failures and 10 inferred unsupported skips. The existing real
Guest has a 1 MiB Tool response limit; stateful 100k and every mixed window
containing it are missing, without pagination, payload truncation or a larger
ABI. Preserve original failed status and runner on WSL. All primary and
ordinary Python request cells complete: 54,456 formal request/wait trace rows,
324 prepared-owner and 9 process-startup diagnostics, plus separately counted
initialization. Full reduction validates 88 shape / 64 mixed / 184 waiting
memory cells, round/window accounting and all 23 frozen source hashes.

Actual Linux fixture-reference acceptance passes 66 tests, including 14
interpreter regressions; five harness invariants also pass under CPython/PyPy.
Cold interpreter leads scalar/100-row/weighted 10k, while native already leads
stateful 100k cold and reused long scans. Nominal 1 ms scan-mix cold short p99:
native 22.69 ms, interpreter 8.13 ms; warm p99 is about 8.3 ms for both, native
uses less CPU. Shared-code 32-task record_wait PSS: native 7.98 MiB,
interpreter 9.70 MiB, common-body Monty 223.78 MiB. Requested owner release does
not imply OS page reclamation. Ordinary Python and the old sync/JSON Guest
adapter remain separate lifecycle/representation cohorts, without a global
ranking or cross-host pooling. No default strategy or production changes.

Full Chinese report, compact identities/summary and rendered SVG:
[wsl-frozen-benchmark.md](wsl-frozen-benchmark.md), `bench/results/s-wsl/`,
`tools/s_wsl_{campaign,secondary,summary,plot}.py`. Task tree is about 2.49 GB
on WSL, raw JSONL about 1.57 MB; neither targets nor raw were copied to Mac/Git.
Six delivered-file hashes match remote read-back. Old Pysolate worktree remains
unchanged. The WSL host may be switched off after delivery; raw remains on its
persistent task disk and has no independent archive.

### S focused review rework: execution complete, pending review

- Added only missing weighted/stateful sizes to reach N=100/300/1000/3000/10000/30000/100000, retaining original successful scalar/five-shape raw and original `speed`/old Guest binaries. 780 extension commands reconcile to 762 completed + 18 explicit old-ABI unsupported; 38,100 scheduled request rows = 37,902 observed + 198 unsupported. No other failures.
- Added true 115-byte x86_64 handwritten checked scan in the benchmark-only assembly reference. Paired Rust/asm kernel-only and full-request boundaries have 56 aggregate cells, 3 independent rounds each. Weighted100k kernel asm/Rust 135.25/91.58 µs, full 372.51/362.91 µs; no asm-optimal or unknown-source-compiler claim.
- Cold native/interpreter mean crossing is bracketed by 30k–100k in both fixed scans; native warm means are lower at every point. Stateful remains predicate-dominated. Preserve cohort markers and observed ranges, no continuous routing-threshold claim.
- Same-process CLOCK_MONOTONIC first-request phases on 8 routes × 3 shapes × 3 new processes: 72 observations. Launch/init/prepare/execute/response-reap-unassigned sum exactly to each total. Scalar means IR 2.80 ms, native 4.30 ms, Monty 3.29 ms; no sum of cross-experiment medians. Guest compile/instance internals opaque to public Runner.Run remain explicitly inside execution.
- WSL assembly tests 5/5; extension CPython/PyPy 4/4 each; original harness 6/6. All 534 original raw hashes retained, original reducer rerun gives byte-identical summary `aa67b4961f584a33cea57f082b89d1c2039db131cd51bf8bee4481e9a2883715`. All 762 new raw/phase identities postflight verified. Four SVGs checked in Chrome. No Mac build/download, runtime/fixture/demo edits or global changes.
- Commands: `python3 tools/s_wsl_extension.py first|reference|curve ../extension` sequentially; `python3 tools/s_wsl_extension_summary.py ../extension ../results/summary.json ../extension`; `python3 tools/s_wsl_extension_plot.py ../extension/extension-summary.json ../extension`. Build/test/source archive steps and limits in `docs/wsl-frozen-extension.md` / `bench/results/s-wsl/extension-verification.json`.
- Raw/build/runtime remain `/home/na/benchmarks/minipy-s`; extension directory ~5.0 MiB. Mac/Git retains small evidence only. Original interpreter_ab source archived separately because the benchmark entry adds first-request timestamps; runtime/fixtures/manifests stay frozen.
- Requested independent gpt-6-luna Codex lane was rejected by ChatGPT account (400 unsupported). No substitute model or claimed child result; parent implemented the bounded benchmark additions.
- Final fixed-fixture admission uses compile-time exact byte assertions. Original measured reference harness separately archived; guarded ELF has a distinct hash, but complete executable `.text` matches measured binary byte-for-byte (SHA `24911366c36aaac77580341ef2bf678c67fa722cab22ba35df82010befdf2697`). Fresh 5/5 tests plus exact/mismatched fixture-copy rustc probes pass. No repeated campaign or changed source/runtime fixtures.

## U browser interpreter and bounded debugger (2026-10-14)

The browser target reuses the existing bounded frontend, sealed tape lowering,
checked dispatcher, output formatter and capability/budget checks. An opt-in
`debug-runtime` specialization retains scratch for actual instruction stops;
native default retains stack-local scratch and has no debugger module or
serialization. Cranelift/native async execution is target-excluded from Wasm.
No guest-language, native strategy or service-worker/permit contract changes.

The trusted Quote fixture provider completes asynchronously in a module Worker.
Eight independent task slots share prepared units; the Worker schedules bounded
turns and two-outstanding-batch ACK backpressure. Snapshots carry real Wasm frame,
IR tape and input addresses, byte sizes, scalar state and small previews. Row,
instruction and write events derive from execution. Cancellation drops inputs
and ignores detached late completion; reset invalidates old delivery epochs.
Terminal debug frames and prepared tapes stay until reset and are explicitly
documented as retained, while linear memory capacity is distinct from live ranges.

WSL Rust1.96.0 `wasm32-unknown-unknown` ran real compilation under the shared
`/home/na/benchmarks/minipy-s/compute.lock`; no heavy Mac toolchain/download.
TUNA's old default 1.87 target returned 404; the already installed 1.96 toolchain's
target was installed from official distribution, with no replacement compiler
or blind repeated build. Source/build stays `/home/na/projects/minipy-u` and
the reused WSL target stays `/home/na/benchmarks/minipy-s/source/target`.
`scripts/build_playground.py` emits a stripped static package and source/compiler
manifest, with a 256MiB Wasm maximum. Artifact paths and exact final sizes/hashes
are in `bench/results/u-playground/verification.json` and package manifest.

Mac `cargo test --tests --features debug-runtime,fixture-reference` passed 72
tests, including six debugger invariants; default tests passed63 earlier.
`cargo clippy --features debug-runtime --lib -- -D warnings`, formatting, JS
syntax and real Wasm import/export inspection pass. Chrome's actual module
Worker/Wasm acceptance passed 31 checks, including editing source -> changed
output, PC/scratch, three tasks, timed wait/resume, chunk yields, last-use input
release, late cancellation, errors, lossless i64 and transport backpressure.
The first acceptance failure was a 75ms test wait racing actual timer delivery;
a bounded state-based await replaced that timing assumption, without weakening
the required ready-state invariant. Browser command timings include ABI/debug
projection costs and are not native benchmark scores.

Engine integration and reproduction: [playground-engine.md](playground-engine.md).
Three editable samples are in `web/samples.mjs`; V owns the Canvas workbench,
responsive UI, final site-wide browser QA and deployment. U adds no web framework,
network Tool, simulated trace or unrelated engine optimization.

Publication scan caught dependency panic-location strings with the WSL home
path even after custom-section stripping. Compiler-wide path remapping fixes
the source of those strings; the final artifact has zero host-home/credential
marker hits and repeats the 31 real browser checks. No blind binary rewriting.
One read-only gpt-6-luna review was requested at signed 012216d and rejected by
the Codex ChatGPT account with 400 unsupported. No substitute model or independent
verdict is claimed; the parent performed scoped self-review and actual gates.
The exact failed lane is retained in `bench/results/u-playground/focused-review.json`.

## AH: static Tools, borrowed JSON and pooled HTTP (2026-10-15)

Opt-in `tool-json` adds Host-declared numeric Tool IDs and at most two i64
arguments, borrowed Bytes roots/static field paths, repeated-path cells and a
shared bounded HTTP pool through the existing frontend/native/tape pipeline.
No general object/array language, new backend, automatic strategy or website
claim. Source argument order, permissions, error order, short circuit, owner
last use and detached-operation cancellation remain explicit contracts.

The bounded macOS M4 experiment retains 297 timing rows over 33 cells, 33
separate allocation diagnostics and 72 paired old-fastpath rows over 12 cells.
Few-field lazy parsing wins on the resident valid fixture; full-array typed
serde/sonic decoding (~33 us) beats lazy/row-slots (~128–158 us), so array
expansion stopped. Repeated lazy accesses need caching. Prepared JSON native
and tape are close; native cold preparation costs much more. Native leaves
still pay shared JSON helper parsing/cache/type/error work and crossings, not
zero-cost compiled JSON access. See [tool-json-results.md](tool-json-results.md)
and [bounded contract](tool-json-view.md) for exact boundaries and raw paths.

Adding Tokio networking changed `enable_all` reactor behavior in the initial
comparison. Preserve that observation separately; final ready-provider pairs
explicitly enable only time in both arms. HTTP still enables I/O and pays its
scheduling costs. No yield, permission, fuel or error check was removed.
Scalar/weighted optional-feature regressions remain visible; no universal
zero-overhead or native superiority claim.

Close-out reran all 86 all-feature tests and all 63 default tests on macOS,
including 10 new Tool/JSON and four real localhost HTTP tests. An earlier
card comment's 96-test count was a transcription error, not a separate result.
Strict all-target clippy, fmt and the existing Wasm/debug library check are
verification gates. Measurements were not repeated in this close-out. Reuse
`CARGO_TARGET_DIR=/Users/yuzhe/projects/minipy-native/target` with one build job;
the close-out initially missed that variable and created a redundant 1.3 GiB
workspace debug target, then removed only that redundant target. Frozen `bench/`
sources and AF/WSL artifacts are untouched. Submit a signed branch snapshot for
one focused review before any main merge.

## AJ: bounded source preparation candidate and direct Tool output (2026-10-15)

Based on approved opt-in AH f7167add, not merged main. The single candidate
removed transient ToolSchema clones during binding; four AB/BA/BA/AB pairs
showed JSON cold changes of +1.5–2.6%, with repeated cold slower in every
pair. Reverted it and stopped optimization, retaining the patch/raw evidence.
Added only ordinary Python `print(tools.name(...))` for declared i64 replies:
frontend expansion reuses Tool suspension and bounded Print, without new
runtime instructions, owner state, helper ABI or strategy. JSON print,
nested effect expressions and Tool-in-loop remain rejected.

Small pre-AH/default/opt-in controls did not reproduce the prior 20–33%
scalar-cold regression; they do not erase AH's negative result or prove
zero overhead. Same opt-in executable with I/O enabled still adds about
14 us to prepared 1000-row ready-provider scans at default chunk=256.
No fairness/cancellation/budget check was changed to reduce that cost.
Three actual localhost HTTP processes validated both syntaxes with real
fetch/publish waits, output, argument, pool/permit and response-count checks.
Direct/assigned tape logical bytes (990) and native code bytes (492) match;
these are not RSS/PSS or measured peak memory.

Final macOS verification: 89 all-feature and 63 default tests, strict clippy,
fmt, Wasm/debug lib check. See [aj-bounded-results.md](aj-bounded-results.md)
for source identity, costs, boundaries, commands and all 2217 small batch/
diagnostic rows; `python3 tools/aj_verify.py` recomputes summaries exactly.
No frozen benchmark, AF/WSL workload, main, website or dependency change.
Signed independent branch is submitted for focused review, not auto-merge.

## AL: I/O scheduler attribution and bounded negative result

Built on accepted AJ a780931 (AH f7167ad ancestry verified), not main.
Production src and Cargo files remain identical to AJ. One Host-only candidate,
Tokio current-thread event_interval 61 -> 256, had no stable net gain and is
not adopted. Both arms kept I/O enabled, default chunk/fuel, yields and owners.
Final mixed/native prepared 80.415 -> 80.325 us, cold 390.423 -> 385.118 us;
old native weighted cold 153.397 -> 157.905 us. Short sibling means, all paired
directions, and the earlier discovery noise are retained, not screened out.

Isolated source-path investigation refines the previous ~14 us attribution:
three bare yields cost IO/time 1.272/0.262 us, two detached oneshot handoffs
0.981/0.366 us, and their combination 14.734/0.623 us. Deferred wake -> root
unpark -> Mio kqueue and idle park_timeout paths are documented, but there is
no syscall trace or claim that individual syscall timings were isolated.

Real loopback mixed scan/scalar/fetch->publish->print runs validate response
counts, output, arguments, permits and connection reuse. 984 canonical batch
rows across discovery/final/isolated probes exactly recompute. macOS 89 tests,
strict all-target/all-feature Clippy and fmt passed. Existing shared target,
offline/locked single job, free disk ~12 -> 11 GiB and shared target ~12 -> 13
GiB after debug verification, ~100 KB raw/summary/log evidence.
See [al-io-results.md](al-io-results.md) for commands and measurement boundaries.
No second candidate, new scheduler, frozen bench, website or main merge.
Focused review launch at integrated 2617a94 failed because Codex CLI's ChatGPT
account rejects gpt-6-luna. No substituted model or review approval claimed.
Route one independent pinned-model review through the Hermes Kanban lane.

## AN: standalone declared scalar Tool expression statement

The existing private, read-only survey input path and cutoff were reused; no
source text, arguments, session identifiers or raw code were exported. Its 632
unique main scripts were scanned by AST for bare `Expr(Call)` / `Expr(Await(Call))`
statements (print excluded). 79 call-shaped statements were found: one direct
`apis` Host-namespace call, 59 attribute calls with an unverified root, and 19
unclassified name calls. This establishes one clear Host-effect example, not a
frequency or runnable-rate estimate; top-level-await coverage is limited by the
survey parser's ordinary `ast.parse` mode. Source categories were aggregated only.

Added only the opt-in frontend lowering: reuse declared-tool binding and existing
`Stmt::Tool` with an unnameable NUL-prefixed result slot; i64 result is discarded.
JSON result discard, unknown Tools, `**kwargs` and Tool-in-loop remain rejected.
No runtime, IR, opcode, backend or permission/budget contract changed. Dual-backend
tests cover positional/reordered keyword calls, explicit top-level await, output,
authorization denial, call budget, argument overflow before dispatch, provider
error, cancellation/late completion and rejection boundaries. The local HTTP
fixture/pool also executes standalone publish on both backends before print.
This feature is an
incremental Host-tool syntax slice, not general Python support.

### Bounded standalone-vs-assigned-discard probe

Ran the existing `examples/aj_http_probe.rs` three times as independent
processes against its loopback HTTP fixture and shared connection pool. The
first three fixed-order runs (`run1`–`run3`) are retained as historical but
superseded data. The corrected runs use AB/BA/AB process ordering, selected by
`AN_ORDER` (default AB; BA only for process two). Both
arms use the same binary, source-derived fetch→scalar-publish program, actual
HTTP requests, and stdout consumption; each timing batch checks response count,
`27\n` output, one reused connection, zero copied bytes and full permit return.
The standalone arm discards publish's i64; the control assigns then prints it.
`source_cold` includes prepare/compile, execute, stdout consumption and unit
cleanup; `prepared` explicitly excludes preparation. Each cell has three
sub-batches per process (20 cold or 100 prepared invocations; nine batch means
per arm overall). Corrected raw CSVs are
`bench/results/an-probe-run{4,5,6}.csv`.

Reproduce: `CARGO_TARGET_DIR=/Users/yuzhe/projects/minipy-native/target
CARGO_BUILD_JOBS=1 cargo run --offline --locked --features tool-json --example
aj_http_probe`, then run the binary with `AN_ORDER=AB`, `AN_ORDER=BA`,
`AN_ORDER=AB`, saving stdout to the three raw files. Source hash
`e7b590bfed570a01bce2ab0aeb04aab54b4d4a18d443bfdb1eddb3f29c8b2848`;
rustc/cargo 1.93.1; binary SHA-256
`095555fd53cc0d475802537e23d21ecc08c8e2e51e43a97e634eb8d0b33c80d5`.
On this macOS M4 probe, median-of-nine-batch per-call ns:

| Backend / lifecycle | Assigned | Discard |
| --- | ---: | ---: |
| tape / source_cold | 300,241 | 271,825 |
| native / source_cold | 1,778,262 | 1,743,506 |
| tape / prepared | 147,294 | 147,449 |
| native / prepared | 146,313 | 145,841 |

These are small-fixture observations, not a performance claim. Each process
runs every assigned cell then every discard cell in AB, reverses the groups in
BA, then returns to AB. They satisfy the bounded three-process ordering; cell
grouping and small samples still leave substantial warm-up/noise limits. No
timing-based optimization or runtime change is made; Linux remains unrun.

## AO: concrete-operation kernel selection and admission gate

Based on accepted AN 79bfcf88, without merging main. Reused the private Z
extractor with exact original sampling/replay/input and aggregate reconciliation:
632 distinct main scripts (Hermes577/AppWorld55). New devalued AST aggregates
distinguish JSON decode/encode, literal sibling/path syntax, string methods,
filtered/projected comprehensions and unresolved reductions. Frequency is not
type proof, execution CPU or compatibility. No corpus source or identifiers
are exported. See [AO selection](ao-kernel-selection.md) and
`evidence/ao-kernels/operations.json` for definitions and six ranked work ledgers.

No performance candidate passed the value-contract/net-work gate. Existing
JSON i64 paths already cache prefixes/leaves; eager multi-field extraction
would violate demanded-error/short-circuit boundaries. Higher-value string,
collection and output candidates need explicit value/ABI decisions. Quote
reduction is technically possible but no sample-to-Quote/i64 demand or net
benefit is established. No kernel, production-code change or timing result is
claimed; the permitted design-only exit is used rather than an unmatched spike.

Mac verification: eight synthetic analyzer tests, byte-identical repeated
aggregates, 14 concrete admission cases on each real compiler (one test), and
18 existing Tool/JSON/actual HTTP execution tests. Scoped strict Clippy and fmt
pass. Reused the existing target offline with one job; no new target, dependency,
WSL/VM work or benchmark campaign. Proposed next experiment is one bounded
read-only JSON string predicate after Host schema/ownership/byte-work decisions,
not a generic Value system or automatic follow-up development card.

## AR: declared JSON string leaf + literal `in` bounded experiment (2026-10-15)

Starting at AO `3b6b800`, added exactly one natural string predicate:
`data = tools.fetch(batch_id); text = data["message"]; print("error" in text)`.
`ResultSchema::JsonTyped` identifies individual `I64`/`String` leaves; old
`Json` remains i64-only. String values are neither printable nor Tool arguments
nor general locals; only a literal UTF-8 needle (<=256 bytes) with an explicitly
declared string leaf/alias is admitted. Dynamic/tuple needles, methods, raw
JSON output, arbitrary string expressions and arrays remain unsupported. The
compiler stores bounded literal needles in the sealed plan; native JIT and
interpreter both use the same borrowed JSON implementation. Literal direct
access and an alias through an async Tool wait run with real source and result.

The immutable `Bytes` owner holds the selected JSON range; unescaped strings
borrow, escaped strings decode once into a per-task cache. Selected missing,
null/nonstring, malformed escape and invalid UTF-8 become task errors at the
access point. First duplicate key follows the existing sonic path semantics;
unvisited fields are not eagerly parsed. The string leaf cap is 64 KiB encoded
bytes, default cumulative per-task validation/decode/search budget is 1 MiB
(engineering admission choices, not Python or JSON limits),
configured via `ScriptLimits::string_scan_bytes`. Decoded *capacity* joins
existing root/quote aggregate buffer accounting, and releases with the owner;
temporary decode allocation is bounded by the 64 KiB leaf cap before the
postallocation capacity check. Unescaped tasks do not allocate decoded
payloads; non-string JSON tasks do not allocate a decoded vector but their
boxed state grows from 80 to 128 bytes (+48 on 64-bit). No per-row scanning or
branch eager read.
Errors retain earlier stdout, and cancellation/late Tool completion drops the
root. No output/Tool byte ABI, generic `Value`, or extra dependency in the
production crate.

Verification: both real backends against literal UTF-8/escaped/empty/duplicate
cases and a `serde_json` content oracle where duplicate semantics agree;
negative admission, mixed i64/string schema, missing/null/wrong type, malformed
escape, short-circuit error order, stdout prefix, 64 KiB/scan/buffer caps,
actual wait with observed owner drop on cancellation, plus a real loopback
HTTP JSON response. Full `--all-features` and no-feature Rust suites, strict
all-target Clippy, fmt and diff-check passed on macOS; Linux/WSL not run.
`wasm32-wasip1` `--lib --no-default-features` check passed (existing unused-code
warnings); feature `tool-json` cannot cross-build on this target because its
pre-existing Cargo feature enables `tokio/net`, rejected by Tokio on wasm.
No wasm guest execution or string-path validation is claimed.

Bounded operation probe (two processes, 30 AB/reverse-order repetitions each,
60 per case/engine/mode) at `evidence/ar-contains/{run1,run2}.jsonl`; validated
40 groups/2,400 raw result rows and summary at `summary.json`. Runner and
source: `bench/src/bin/ar_contains.rs`; bench feature `string-probe` uses the
already pinned Monty rev 56949a4 and rustc 1.96.0 (required by Monty, installed
locally; default project rustc 1.93.1). Run command:

    CARGO_TARGET_DIR=/Users/yuzhe/projects/minipy-native/target CARGO_BUILD_JOBS=1 \
      cargo +1.96.0 run --offline --locked --release --manifest-path bench/Cargo.toml \
      --features string-probe --bin ar-contains -- 30

Each timed invocation includes source compilation in cold mode (prepared
runner cloned in warm), async Tool hop, JSON handling, guest execution, output
consumption and cleanup. Provider starts with immutable JSON `Bytes`; there is
no HTTP wait in this probe. Monty `await fetch` creates a dict/string by parsing
the same JSON *inside* the timed Host callback; `await contains_message` is the
strong simpler Host-bool baseline (also parses the JSON inside the callback).
The latter moves the predicate into the trusted Host and has a different Tool
return contract, so it is a decision comparison, not an implementation of the
same guest interface. Separate loopback test checks the HTTP integration.

Median wall ns (small hit / 16 KiB tail hit / 16 KiB miss / escaped hit / 64 KiB miss):

| mode | native | interpreter | Monty dict | Monty Host bool |
|---|---|---|---|---|
| cold | 181917 / 152104 / 124583 / 102666 / 120479 | 21999 / 28124 / 23854 / 21792 / 38375 | 21812 / 21979 / 16625 / 16729 / 22646 | 15895 / 15833 / 13354 / 13312 / 18687 |
| prepared | 1624 / 8146 / 8041 / 11875 / 28687 | 1687 / 9125 / 8937 / 11792 / 29520 | 5625 / 8020 / 8770 / 10250 / 15125 | 5208 / 6792 / 7250 / 9666 / 13354 |

These are local synthetic payloads, not observed application latency or
network-inclusive measurements. Native prepared small-hit beats Monty dict
~3.46x, but cold native is ~8.34x slower and long/escaped prepared results do
not consistently beat the strong simple baseline. Thus there is no blanket
kernel speedup: this branch is a narrow correctness/performance candidate for
review, not a default deployment or justification for more string operations.
If the target workload is cold or large/escaped strings, prefer Host-bool or
the existing interpreter rather than expanding this slice. The 64 KiB prepared
whole-task native path reached a 33 µs maximum over 60 runs on this host;
this is an observation, not a cancellation-latency guarantee or proof for
arbitrary payloads beyond the admission cap. No CI/VM, new
machine, global changes, or production dependencies were used.
The dependent AS integration card owns paired old scalar/scan regression and
candidate keep/drop against the combined branch; this AR probe alone is not
evidence of zero regression for the other workloads.

## BC: fresh-source request, five arms (2026-10-15)

WSL2 CPU2, serial under existing `compute.lock`: MiniPy interpreter/native,
Monty, fresh CPython process and prewarmed CPython prefork receive 30 distinct
semantic source variants per shape (one shared across arms for each request).
The initial `%12` campaign only had 12 variants per shape; its unmodified raw,
summary and startup are archived in `bench/results/bc-wsl-v1/`, not pooled.
Same controller timer spans source transmission, prepare/compile, data
generation/conversion, real Tools, stdout delivery and child reap where
applicable. Source generation/oracle are outside the request timer. Prefork
parent initialization is separately measured, never amortized invisibly into
per-request latency. The corrected 15/15 real smoke and 1200/1200 formal
requests passed; all eight shapes have 30 distinct source strings, and 40
summary cells independently recalculate from raw.
Seven weighted sizes and scalar only; old resident/COW/other curves were not
mixed. See [BC full method and results](bc-fresh-source-benchmark.md),
`bench/results/bc-wsl/{raw.jsonl,summary.json,startup.json}` and
`tools/test_bc_campaign.py`. On weighted 100 / 100k the five mean request
latencies (µs) were respectively 269/2437 interpreter, 879/1525 native,
486/274904 Monty, 65235/157892 spawn, 4548/98822 prefork. No production
runtime, website, frozen S/AD raw or global system setting changed.

## BG: cached validation for borrowed JSON string spans (2026-10-15)

The opt-in `tool-json` path previously reparsed a plain JSON string token with
`sonic_rs::from_slice::<&str>` on every `contains` after locating the field.
`src/json_view.rs` now records `Unchecked` / `Validated` / `Decoded(String)` per
path. The first plain-string access still uses sonic to validate UTF-8, controls
and JSON syntax; subsequent reads borrow only the checked token's interior from
the immutable `Bytes` owner. Escaped strings keep the previous single decode.
Replacing or releasing an owner clears the marker and all associated range/
integer/decoded state. A failed replacement preserves the old owner. The same
64-KiB token, task buffer and cumulative scan charges remain; no new syntax,
public ABI, dependency or non-JSON allocation. `State` remains 128 B on 64-bit.
The opt-in string-path vector still uses 24 B per path on macOS ARM64:
`StringCache` fits its two marker variants into the niches previously used
by `Option<String>`. This is a Rust layout test, not an observed waiting RSS
delta; no gated task-memory campaign was run.

Bounded paired macOS M4 probe: exact HEAD `4e54261` versus this change, Rust
1.96.0 aarch64-apple-darwin, release, pinned Monty revision unchanged. The
existing `ar-contains` harness was temporarily patched to compute a threefold
short-circuit hit over the same string; both binaries used identical probe
source and Tool JSON bytes. Misses evaluate one predicate. Monty and Host-bool
rows still run their original one-predicate sources and are sanity checks only,
not matched threefold comparators. Four adjacent AB/BA/BA/AB process pairs,
60 timed requests per shape/engine/mode/process, 19,200 result rows and eight
identity rows. All results matched the expected stdout. Compilation, Tool hop,
JSON access, execution and cleanup are inside cold timing; prepared units are
outside warm timing. The immutable producer bytes are precreated per shape.
Both compilation and timing held the shared `minipy-perf-build.lock`, with
`CARGO_BUILD_JOBS=1` and the existing shared target. An earlier unlocked pilot
was discarded, not pooled. Median pair changes for native / tape respectively:
prepared 16-KiB tail-hit native -59.75/-52.89/-58.25/-53.28%, tape
-55.16/-51.81/-54.91/-55.09%; prepared 16-KiB miss native
-37.38/-32.27/-38.64/-38.34%, tape -25.99/-30.77/-33.42/-30.38%;
prepared 64-KiB miss native -32.94/-33.05/-33.48/-32.90%, tape
-32.54/-32.97/-34.58/-32.83%. The 16-KiB tail-hit source-cold native
improves 3.56--6.09% in all pairs; cold short-hit varies
-0.51/+0.17/-0.84/-2.02%, and escaped-hit is neutral/noisy. These are local synthetic cells, not a representative mix
or a Linux/WSL run. Retain this local optimization for repeated plain-string
access without asserting a general short-task or service win.

Old non-JSON path control, same shared lock/target, ordinary `script-bench`
without `string-probe` or allocator diagnostics: 4 AB/BA/BA/AB process pairs,
220 cold requests per process/shape, first 20 excluded, scalar (zero rows)
and weighted Quote scan (100 rows). Paired scalar median changes
-5.10/-3.68/+2.69/-0.34%; weighted -1.29/+0.38/-0.74/-1.52%.
An earlier 80-request control had large initial frequency drift and was
discarded. The longer result does not establish an old-path speedup, but
shows no stable repeated net loss in these cells. Full raw and reduction are
`old-final-*.jsonl.gz` and `old-path-summary.json`. The common baseline and
candidate script binary SHA-256 are
`3d8ddb8c65593bd25e9dae7b72da484c3fc8c9eeacaf642a14522f2d39e12c1c` /
`136335e783e5198da6e621aadf6349888bea19c2d3aca4c20c39743e7001894c`.
The non-string plan still allocates no string-state vector; its inline State
size is unchanged. No gated task-memory campaign was run.

Raw gzip, per-pair reduction and probe-only source patch are in
`evidence/bg-string-validation/`. Baseline/candidate repeat binary SHA-256:
`91de87d921abc0a62dc4901522da16defda781d0192c6eb9f1150703fac4d3dd` /
`12512ef4b3d22c2183bb73e888773a5d42f8d6ed715e80f8b4c65003ba23d98c`.
Reproduction: decompress and apply `repeat-source.patch.gz` to each of an exact-HEAD worktree
and this tree, build with `CARGO_TARGET_DIR=<shared-target> cargo +1.96.0 build
--offline --locked --release --manifest-path bench/Cargo.toml --features
string-probe --bin ar-contains`, save distinct binaries and run each with `60`
in the recorded order. Unit replacement/invalid-token/owner tests and full
macOS `cargo test --locked --features tool-json` plus
`cargo test --locked --no-default-features` passed; strict all-feature Clippy,
`cargo fmt --all -- --check` and `git diff --check` passed. No Linux run, CI,
VM or new machine was used.

## BH: selective integration decision (2026-10-15)

BF's two record-isolation candidates remain reverted on its reviewed branch:
C1 saves 632 allocator-requested bytes across 16 Quote waiters without a stable
request improvement; C2 improves both local paired Quote runs, but the cause
is not isolated and emits an additional execution body. BD's rows borrow and
field-locality work is not imported with BG; the old Quote warm gap remains
unresolved. BE's JoinHandle substitution also remains reverted. No HTTP
single-job prototype was justified: the observed I/O-driver interaction is
not an isolated task/channel deletion opportunity, and the proposed Host
entry, handle-drop and permit semantics have not been implemented or measured.
Typed scan kernels and local JIT lack a representative admitted workload.

Keep only BG's independent opt-in string validation cache. To test the
natural alias + **single** contains (rather than BG's triple-hit source),
`tools/bh_compare.py` built `ar-contains` from main `4e54261` and BG
`f17cf8e`, both with the original unmodified `bench/src/bin/ar_contains.rs`.
Build and all six process runs held `minipy-perf-build.lock`, shared target,
jobs=1; Rust 1.96.0 release on macOS M4. Three adjacent pairs, order
AB/BA/AB (40 measured requests per case/engine/mode/process), returned the
expected stdout for every call. The probe includes fresh source compilation,
Tool handoff, JSON access, guest execution and cleanup in cold timing; its
producer holds already-encoded immutable Bytes, with no real network wait.
Both arms execute identical script and payload; Monty and Host-bool controls
are present but not paired comparisons for the integration decision. Raw files
`evidence/bh-string-integration/{0-base,1-candidate,2-candidate,3-base,4-base,5-candidate}.jsonl.gz`,
summary, SHA256 binaries and exact source assertion are retained with the
runner. To rerun after integration, create isolated worktrees at the two
recorded commits and set `BH_BASE_TREE` and `BH_BG_TREE`; the runner rejects
other revisions. This is an additional small paired check, not a full campaign.

Per-pair relative changes (candidate vs main) for source-cold tape:
16-KiB hit -15.48/-12.81/-12.93%, miss -24.32/-12.29/-9.53%, 64-KiB miss
-25.96/-25.41/-19.05%. Cold native 64-KiB miss is
-4.87/-11.17/-6.88%, but 16-KiB hit and both engines' short hit are not
stable; the first process pair also has much higher cold levels than the
subsequent two, so its changes alone are not a general speed estimate.
Prepared 16-KiB hit improves 22--31% on tape and ~30--31% native in all
three pairs, consistent with removal of repeated validation after the alias.
Escaped strings are neutral/noisy; short warm native slightly regresses in
these three pairs (3.21--5.86%, about 40--80 ns) and has no demonstrated
source-once gain. These local medians neither describe service load nor prove
that every short case benefits. BG's earlier four-pair old scalar/Quote control
has no consistently directed loss. State remains 128 B and per-string-path
storage 24 B versus BG's parent; no gated RSS comparison was run, and the
entire root can still be held by a small live string across an I/O wait.

Decision: integrate only BG's independently reviewed cache and its error/
owner regression tests; do not infer a generic long-string workload or
broader feature expansion. Mac all-feature and no-default suites, strict
all-target/all-feature Clippy pass offline under the same lock, plus default
toolchain fmt check. No new dependencies, VM/WSL, CI, website, paid resources
or release automation. HTTP operation ownership, cancellation and budgeting
remain unchanged. Future HTTP work needs a measured real handoff cost and a
bounded service entry design; no new optimization task is opened here.

## BI: bounded builtins and source admission (2026-10-15)

Source admission remains one RustPython parse and typed lowering, not a Python
runtime. AO's `evidence/ao-kernels/operations.json` counted syntactic presence
only: its 520 exact-unique `hermes/cli` sources contain 6 unresolved `min`
and 15 unresolved `max` syntactic presences, often with generators; these
counts cannot establish i64, quote-buffer provenance or full-source
compatibility. The low-complexity admitted slice is
direct `len` of a loaded Quote/v1 buffer (already bounded by MAX_QUOTES),
checked i64 `abs`, and two-operand i64 `min`/`max`. They use the existing typed
IR and both native/tape backends. `len` reads validated view/owner length
without materializing rows or spending row fuel; future Tool waits retain the
owner through existing liveness. `abs(i64::MIN)` yields Overflow before any
later effect. Builtin identifiers cannot be rebound. String/JSON len,
iterable min/max, generators, `sum`, `any`, `all`, `sorted` and arbitrary calls
remain rejected. No general dynamic `Value` or iterable protocol was added.

`CompileError` now exposes `kind` (Syntax, Unsupported, Type, Schema, Backend),
`reason`, and optional one-based `(line, UTF-8 byte column)` location. AST
expression and statement errors report the innermost known source node;
Host input/Tool schema and backend failures have no source location. Earlier
stdout is only relevant to an admitted program at runtime: rejected source
never begins a task and cannot dispatch a Tool. `tests/builtins_admission.rs`
checks acceptance, overflow ordering, short-circuit, empty/nonempty buffers,
wait retention and rejection on both backends. The AO corpus is private and
no raw trajectory is copied into this branch; the synthetic rejection examples
are shape tests, not claims that sampled scripts run end-to-end.

Mac aarch64 Rust 1.96.0: `CARGO_TARGET_DIR=<shared target> CARGO_BUILD_JOBS=1
cargo +1.96.0 test --offline --locked --features tool-json` passed (including
real localhost HTTP tests), and strict all-target Clippy for that feature
passed under `minipy-perf-build.lock`. `--features debug-runtime --test
debug_runtime` passed. The `wasm32-unknown-unknown` target is listed for the
default toolchain but not installed under Rust 1.96.0; an attempted offline
check fails at missing `core`, so no Wasm validation is claimed. No complete
natural-source admission gain or Linux run is claimed.

Small old-source control: `git worktree add --detach baseline 89b898b`, then
under the shared `minipy-perf-build.lock`, for each arm in B/A/A/B run
`cargo +1.96.0 clean --release --manifest-path ARM/bench/Cargo.toml -p
minipy-native -p minipy-native-bench`, `cargo +1.96.0 build --offline --locked
--release --manifest-path ARM/bench/Cargo.toml --bin interpreter_ab`, and
`python3 tools/bi_measure.py {base|candidate}
evidence/bi-builtins/source-cold.jsonl`; `ARM` is `baseline` for B and `.`
for A. Cleaning the two packages on the *shared* target between arms is
necessary: Cargo otherwise reuses the executable from the other checkout.
The runner checks completed output and records the exact binary SHA, source
commit and dirty status per request; 100 requests per scalar/weighted ×
native/tape × process, 1600 raw rows in `source-cold.jsonl.gz` (SHA256 of
uncompressed JSONL `cfde32ad3074bba3ceebc243e8c0f2e54d04b92f72994b0a30e66c484d185594`).
Rust 1.96.0 macOS ARM64, uninstrumented release, same ready provider and
source in both arms; scalar has 0 rows, weighted 256. Wall nanosecond medians
per independent process in B/A/A/B order:

| engine/case | B1 | A1 | A2 | B2 |
|---|---:|---:|---:|---:|
| tape scalar | 23,667 | 30,396 | 27,625 | 28,375 |
| tape weighted | 34,354 | 34,791 | 34,916 | 34,333 |
| native scalar | 148,708 | 148,145 | 146,209 | 140,083 |
| native weighted | 191,458 | 182,333 | 184,687 | 184,000 |

Both arms' prepared tape bytes were identical for the old sources (scalar
594, weighted 1090); no builtin-specific runtime state is allocated on old
sources. Scalar tape variance in this short four-process check is too high to
infer a reliable win or loss (A1/B1 +28%, A2/B2 -3%); weighted tape pairs are
within ~2%, native pairs mixed. This does not establish universal performance
neutrality or a new performance claim. It does exercise source-cold preparation
plus execution and cleanup; prepared execution was not separately measured.
The baseline worktree was removed after measurement. No new target or toolchain
was created.

Support matrix and next gaps (source binding, not runtime fallback):

| Shape | Admission | Reason or boundary |
|---|---|---|
| `len(q)` after `q=tools.load_quotes(...)` | yes | Quote/v1 owner/view count; 0-row budget succeeds |
| `abs(x)`, `min(x,y)`, `max(x,y)` with i64 operands | yes | scalar IR, checked `abs` |
| `len(string_or_json)`, iterable min/max | no | no string/JSON length-value or iterable semantic IR |
| `sum/any/all` generators; `range/enumerate` | no | no generator/iteration contract in the finite slice |
| unsupported call/arity/type/syntax | rejection at prepare | typed reason plus innermost AST location, before effects |

Next highest-value *candidate* is typed sequence iteration for genuine
generator reductions, but it introduces iteration and ownership semantics;
AO's syntactic counts alone do not justify it. UTF-8 string length is a
separate bounded scanning design with character-count and error/budget rules;
it was explicitly not implemented as byte length. No generic `in` kernel:
string membership already has a literal-needle substring implementation,
while future sequence membership would need typed element comparisons.
Registration is static at compiler/AOT build, source preparation binds names
and types once, task input installation checks declared scalar types, and
execution retains existing per-task authorization/resource bounds. New
builtins incur no per-row dispatch or dynamic registration probe on old code.

BI review follow-up: top-level admission (`return` placement/value and legacy
`async main` name, signature and result annotation) now attaches a source
location before returning a typed error. Both preparation backends have a
focused regression; Host schema/backend errors remain unlocated.

Private complete-source admission check (read-only, no source export):
`python3.14 tools/bi_private_probe.py` reuses Z's cutoff, CLI time-quantile
session selection, call replay and exact-source dedup; it verifies the CLI
session hashes/replay counters and the complete CLI aggregate against Z's
private saved summary. It picks the lexicographically lowest 64 SHA256 digests
of 520 exact-unique `hermes/cli` `execute_code` strings, sends each complete
source over stdin to `InterpreterUnit::prepare_script(source, &[])` with
`tool-json`, and prints only fixed-category counts. Result: **0/64 admitted**;
32 unsupported calls (`builtin_other` in the probe's conservative fixed-label
mapping; no sampled `sum/all/any/range/enumerate` as first error), 12 unsupported
statements, 8 unsupported expressions, 1 unresolved name and 11 other
unsupported admission failures. These are first-error buckets, not all errors
per program. No task was executed and no Tool effect occurred. The lack of a
declared Host input/Tool schema is part of this specific probe, so this is not
a production-script compatibility estimate; programs can fail for different
reasons when given their actual schema. Nor does 0/64 establish 0/520 or
anything about unseen workloads. The original corpus remains private; no
source, per-script digest, identifier or raw error reason enters the repo.
During this rerun, newly inserted pre-cutoff subagent sessions changed Z's
subagent time-quantile sample, so only the independently reconciled CLI stratum
is reported; the old 632-source cross-stratum aggregate is not reused as a
fresh full-source admission denominator.

## BJ: ablation and static-work ledger (2026-10-15)

Read-only synthesis of existing E–BH reports and experimental-branch summaries:
[short conclusions, detailed lifecycle table and next hypotheses](ablation-summary.md),
with a compact [machine-readable transcription](ablation-summary.json). Each
reported time change uses candidate/baseline minus one; CPU, wall, cold/prepared,
machine and version are separated. No production source, landing page, Cargo
build, new benchmark or raw evidence was changed by this documentation task.
Existing comparisons remain independently scoped; AY/BD/BF/BE withdrawn
branches are not silently promoted to main. The next steps are admission
hypotheses, not authorized performance implementations.

## BK: BI/BJ integration gate (2026-10-15)

Integration branch fast-forwarded the independently reviewed BI commit
`493d9e4` from main `89b898b`, then cherry-picked BJ `164ebd5` as `db5eda0`.
The sole overlap was the append-only tail of this file: both BI and BJ sections
are retained without changing either report. BJ adds no production code.

BI's 64/520 deterministic private CLI complete-source sample admitted **0/64**
with no Host schema (32 first unsupported calls, 12 statements, 8 expressions,
1 name, 11 other unsupported). That probe is read-only and neither the 0/64 nor
AO syntactic presence estimates production acceptance. The admitted functionality
is typed Quote-buffer `len`, checked-i64 `abs` and two-argument scalar `min/max`;
`len(string/JSON)`, iterable/generator forms and general Python remain missing.
Host fallback must be chosen on preparation rejection before any Tool effect;
there is no automatic runtime replay. Syntax/source diagnostics include byte
position when available; Host schema/backend errors do not.

Independent old-source integration control ran **two** B/A/A/B rounds under the
shared build lock on macOS ARM64, Rust 1.96.0 release, default features, ready
provider, with `tools/bk_control.py`. Each arm cleaned and rebuilt the two
packages in the same shared target to avoid wrong-checkout binary reuse. The
script checks completed result, records binary SHA and exact source revision;
each process has 50 fresh-source requests for each of scalar(0)/weighted(256)
× tape/native. Both arms run identical old sources without builtins. The
[1600 raw rows](../evidence/bk-integration/source-cold.jsonl.gz) decompress to
SHA256 `643f3de88d10c3be74aadbb7d77889f780f3fa114e5a3dcb0f250dc6bbada584`.
Each cell below is median wall ns within its process, B = main 89b898b,
A = integration db5eda0; adjacent A/B elapsed changes use
`(A/B - 1)*100%` (positive is slower):

| engine/case | B0 | A1 | A2 | B3 | B4 | A5 | A6 | B7 | adjacent A/B changes |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| tape scalar | 24,083 | 29,459 | 28,146 | 25,625 | 21,209 | 28,334 | 28,292 | 29,562 | +22.32%, +9.84%, +33.60%, -4.30% |
| tape weighted | 38,771 | 43,667 | 39,625 | 38,667 | 25,751 | 43,875 | 43,792 | 43,917 | +12.63%, +2.48%, +70.38%, -0.28% |
| native scalar | 148,833 | 148,583 | 151,792 | 148,813 | 156,896 | 146,896 | 148,854 | 149,625 | -0.17%, +2.00%, -6.37%, -0.52% |
| native weighted | 210,333 | 212,626 | 214,730 | 221,667 | 223,583 | 212,250 | 232,521 | 232,980 | +1.09%, -3.13%, -5.07%, -0.20% |

Tape scalar/weighted changes are mostly positive; B4 is unusually fast in
both tape cases, while A6/B7 are essentially matched. Tape prepare medians
track wall changes closely (scalar +22.78/+9.76/+31.66/-4.98%; weighted
+11.95/+2.84/+71.58/-0.06%). Native is mixed/small. This is an unresolved
source-cold tape *regression risk*, not proof of a stable 10–70% penalty or a
production capacity estimate. The round limit is reached; do not repeat until
convenient. There is no evidence here to eliminate the tape risk. Prepared
execution and service load were not measured in BK. No new target or toolchain
was created; temporary detached baseline worktree was removed.

The concrete cost locus is **tape preparation**, not old-source tape execution:
for scalar, B0/A1 prepare medians are 22,041/27,062 ns while execution is
1,792/2,084 ns; for weighted B0/A1 they are 32,792/36,708 ns while execution
is 5,750/6,667 ns. Both old-source prepared tape lengths remain the same as
BI's earlier control (594/1090 bytes). In the source diff the relevant new
per-AST-node preparation work is `Compiler::expr` and `compile_stmt` wrapping
every error in source-position attribution (`src/frontend.rs`), plus the
builtin call admission branch. That is a **mechanism candidate**, not an
isolated causal proof: Rust compiler layout, scheduling and B4's outlier can
also affect the medians. The single bounded follow-up fix candidate, only if
a targeted trace confirms this mechanism, is to attach source offsets at the
actual rejecting AST leaf and eliminate the per-node `map_err` wrapper on
successful traversal, retaining innermost-location behavior for every error.
This is not a trivial deletion: the current `map_err` already runs `.at` only
on errors, and removing it without another origin carrier breaks diagnostics.
No candidate was applied or represented as verified in BK.

Integration correctness checks under the same lock: `cargo +1.96.0 test
--offline --locked` and `--features tool-json` (including localhost HTTP)
passed; `--features debug-runtime --test debug_runtime` passed 6/6;
`cargo +1.96.0 clippy --offline --locked --all-targets --features tool-json
-- -D warnings` passed. Default-toolchain `cargo fmt --all -- --check`
passed; Rust 1.96.0 has no installed rustfmt component, so its fmt command
cannot run without installing a component. BI's missing 1.96.0 Wasm target
and lack of Linux run remain unvalidated, not converted into successful runs.

**Decision:** keep BI/BJ together as a signed, reviewable integration branch,
but **do not merge main yet**. The genuine functionality and diagnostics are
correct in exercised configurations, while complete-script applicability is
low in this bounded no-schema sample and old-source tape performance has a
plausible nontrivial regression. A later separately authorized, mechanism-led
stage may investigate the frontend/admission/tape code layout with controlled
process affinity/phase attribution and then decide whether to narrow or revert
BI. Do not hide that work in an unbounded benchmark or claim BJ percentages
compose into a new integrated speedup. The existing finite typed helper remains
the strong simple alternative for expressible aggregate workloads.

## BL: resident new-source cost gate (2026-10-15)

**Decision: keep the BI implementation for review/integration, defer any
frontend speed fix.** The finite builtins and structured diagnostics already
passed the BK correctness gate. A new paired resident-process control finds no
repeatable material full-request regression on old source; the earlier BK tape
prepare-only risk is not established as a stable end-to-end penalty. This does
not establish a general admission improvement: BI's private complete-source
sample still admits 0/64 with no Host schema. Main integration remains the
owner's decision after review.

Source audit against main `89b898b`: `Compiler::expr` and `compile_stmt` add
`map_err` around typed lowering. On success `CompileError::at` does *not* run:
its source-prefix scan is error-only, as are syntax-error formatting and
position construction. The successful path does execute the `Result` branch
and the new `ast::Expr::Call` branch (for other expression kinds this declines
immediately); reserved-name comparisons are extended only when declaring or
validating names. The literal builtin roster and typed IR constructors are
compiled into Rust and need no per-request dynamic registration, while parsing
and typed name binding of each *new* user source cannot be moved to startup.
Backend lowering is required for native, and tape emits the same old-source
prepared lengths as BK. No security/budget check was removed.

Diagnostic-only, temporary source ablation: `tools/bl_ablate.py` removed the
two success-path `map_err` wrappers from an A build and restored the source in
`finally`; this diagnostic binary **breaks location semantics and is not a
production candidate**. ANNA processes, 100 unique-source requests per
case/process, tape scalar/weighted medians (total wall µs): A0 8.29/21.29,
N1 24.04/20.77, N2 8.21/21.15, A3 8.21/21.62. The scalar N1 outlier and
weighted differences (~2%) do not identify wrappers as the cause of BK's
large per-process tape dispersion. In particular, source scanning cannot
explain accepted requests. No optimization was implemented.

`tools/bl_control.py` compared B `89b898b` and A `e436812` on macOS ARM64,
Rust 1.96.0 release, offline/locked, shared Cargo target with an `fcntl.flock`
held over each build and all subprocesses. The same harness source was copied
into a detached baseline worktree; each arm cleaned both packages and rebuilt
before copying the actual executable (binary SHA per row). Service setup
(runtime/source selection, **no** user-code preparation) occurs once per
process and is emitted separately. Then 80 requests per case/engine/process
arrive as distinct source strings with a unique comment suffix. The timer
covers provider construction, parse/typed prepare and native compile or tape
assembly, execution and executable reclamation. Correct stdout is asserted
outside the timer; there is no user-source cache. This is a resident executable
with ready providers, not an HTTP server or production workload. The synthetic
builtin cases run only on A because B rejects them; they are functionality
examples, not cross-version speedups.

Two bounded BAAB rounds were recorded: [round 1 raw](../evidence/bl-request-cost/service-cold-r1.jsonl.gz)
(uncompressed SHA256 `fdb85cf3e7a7b29f446173797cd97eb763e62a865a91c718efa240d1d5a40ea0`)
and [round 2 raw with setup](../evidence/bl-request-cost/service-cold.jsonl.gz)
(SHA256 `a7a057945cb6ead49c5dd2d8d1912192de4004fe0c49240e72f7ac47a72c9e08`),
1920 speed rows each. Round 1 preceded a harness-only setup-emission change;
its execution boundary is otherwise the same. The [ablation raw](../evidence/bl-request-cost/ablation.jsonl.gz)
(SHA256 `6f33a7f2381a145a91cc107cdee3f5e7cdc045bcc3ab13eb8d078ee41e328f58`)
has 800 speed rows (four processes × two cases × 100 requests) and predates
that setup emission. Round 2 median full wall µs, in B0/A1/A2/B3 process
order, with adjacent `(A/B−1)×100%`:

| Engine / old source | B0 | A1 | A2 | B3 | A1/B0 | A2/B3 |
|---|---:|---:|---:|---:|---:|---:|
| tape scalar, 0 rows | 35.21 | 29.00 | 8.21 | 8.65 | −17.63% | −5.05% |
| tape Quote scan, 256 rows | 22.29 | 22.65 | 23.73 | 23.12 | +1.59% | +2.61% |
| native scalar, 0 rows | 63.90 | 66.81 | 62.54 | 70.65 | +4.56% | −11.47% |
| native Quote scan, 256 rows | 151.25 | 157.06 | 151.83 | 151.94 | +3.84% | −0.07% |

A-only builtin scalar tape 9.54/8.79 µs, native 74.77/72.65 µs;
Quote-buffer `len` tape 6.00/6.29 µs, native 59.06/60.27 µs
(A1/A2 respectively). Setup usually ~17–26 µs, except first tape scalar
process setup at 559/410 µs in B0/A1; it is *excluded* from request cost.
Round 1 showed similarly strong scalar-tape within-sequence drift (B0 35.13,
A1 28.04, A2 8.21, B3 8.54 µs). In round 2 scalar tape B0 sample 0 was
1,684 µs and its sample 70 ~35 µs, whereas A2 starts ~68 µs and settles
near 8 µs; thus scalar pairs confound process order/warmup and cannot be read
as BI speedups. Weighted tape has a small positive shift in this setting but
not a convincing material full-cost regression. Native pairs change sign.
BK's tape preparation observation is still a risk in other processes/shapes,
not disproved by these four short processes. No third campaign or generic
cache/prebinding machinery is justified. If a later real service workload
shows a persistent full-request penalty, the next useful experiment is a
phase-specific profile of the same source binding path under that workload,
not another unguided B/A repetition.

Reproduction: `git worktree add --detach bl-baseline 89b898b`, then
`python3 tools/bl_control.py` (requires the output JSONL path absent); the
script copies the identical harness into the baseline checkout. For the
diagnostic source ablation use `python3 tools/bl_ablate.py` with the A binary
from the control still present. Remove the temporary worktree afterward.
`cargo +1.96.0 test --offline --locked --test builtins_admission` (4/4),
`cargo +1.96.0 test --offline --locked --manifest-path bench/Cargo.toml
--bin interpreter_ab` (2/2), and default `cargo fmt --all -- --check`
passed under the same shared-target build lock for Cargo. No Linux/Wasm or
service network measurement is claimed.

## BK private complete-source obstacle inventory (supplement to BK)

`python3.14 -W ignore::SyntaxWarning tools/bk_private_ast.py` reuses BI's
reconciled CLI sample selection (the lowest 64 SHA256 digests of 520 exact-
unique complete `execute_code` sources). It parses all 64 with CPython's AST
and `symtable`, traverses *whole scripts*, and prints only fixed labels and
aggregate counts. It neither executes source nor persists source, digests,
call identifiers or per-script outcomes. This is a **static shape inventory**,
not a second admission run or a claim of runnable programs. The prior real
prepare result remains 0/64.

Overlapping buckets in this sample (denominator 64): 6 explicitly import the
known Host helper modules, 45 reference a recognized preimported Host helper,
and 60 reference at least one other global whose binding needs verification
(which is **not** proof that all 60 are Host APIs). Twelve have imports; three
import known stdlib OS/I/O-capable modules and four import a non-stdlib module
(possibly local, not automatically a third-party dependency). These dependency
categories overlap. All 64 contain at least one call shape beyond BI's four
scalar builtin forms; this includes legitimate potential Host calls, so it
cannot alone rank missing Python builtins. Fifty-eight contain at least one
AST language/data feature outside the finite typed slice: 46 f-string,
28 subscript, 19 list literal, 13 dict literal, 15 generator expression,
9 dict comprehension, 11 assert, 2 function definitions (overlap allowed).
Only 6 have none of those flagged AST nodes, but all 6 still have an unresolved
global and a non-BI call shape. Among the 58 with a flagged node, 20 have
*only* f-string from this finite feature list; those 20 also have unresolved
globals (12 recognizable Host references). In the three coarse axes
(recognized Host, module dependency, missing language/data feature), the
64 split into (0,0,0):6, (0,0,1):12, (0,1,1):1, (1,0,1):40,
(1,1,1):5. This table does not infer Python syntax coverage from AST
presence alone: unsupported expressions/calls may coexist in every bucket.

The smallest **next candidate scenario** is a *single declared Host operation
with scalar arguments/result and bounded formatted output*, for example
`value = tools.read_metric(1); print(f"value={value}")` as an explicitly
**synthetic** shape. It combines one finite Host declaration/binding with
restricted f-string scalar interpolation and bounded stdout; it does not need
dynamic list/dict objects or a generic iterable protocol. First obtain and
validate the actual target operation's signature, authorization and failure
contract, then test one *complete* private script unmodified at prepare and
execution. The 20 f-string-only AST shapes make this a plausible narrow
investigation, **not** 20 predicted admissions: their other calls/globals may
require adapters or different types, and the present Tool schema cannot be
invented to increase a score. If no actual target has this contract, stop
rather than implement formatting in isolation. Supporting f-strings as Python
strings also costs typed string values, lifetime/owner or bounded copy rules,
UTF-8/error/size accounting and both tape/native lowering; a simpler
scalar-only `print(value)` control already exists. Avoid promoting a generic
container/module/OS capability stack from frequency counts. This candidate
remains a separate decision, not authorized implementation in BK.

## BM: BI/BJ integration decision (2026-10-15)

BI's finite typed builtins and structured admission diagnostics, BJ's ablation
summary, and the bounded BK/BL integration evidence are integrated together.
The imported source history is `1678c82..92fe643` on top of `89b898b`; no
AY/BD record representation or BF/BE experimental production code is imported.
The BK tape-prepare regression signal remains documented, while BL's paired
resident new-source full-request control did not reproduce a repeatable material
old-source penalty. This is a bounded integration decision, not a claim of
general performance neutrality or improved compatibility: BI admitted 0/64
complete private CLI sources without their Host schema. No private source is
stored in the repo. The raw BL ablation contains **800** speed rows, as its
ANNA × two cases × 100 request runner specifies; the earlier 400-row count was
a documentation error, not a change to the retained raw file or its digest.
macOS validation and remaining Linux/Wasm limits are recorded in the BM card.

## BO: bounded print of an existing declared JSON string leaf (2026-10-15)

BO adds `print(data["message"])` and `print(text)` when `text` is an existing
unconditional alias of a `JsonTyped` string leaf. Static path binding, the
existing borrowed/decoded UTF-8 cache, and task-private `stdout` are reused.
No new Tool argument type, Provider method, string value type, generic literal
print, filesystem capability, or public output shape is introduced. The fixed
Rust helper validates the accessed leaf, checks the remaining stdout *bytes*
including newline, reserves within the task limit, and appends the whole line
or leaves the earlier output intact. Escaped content uses the existing decoded
scratch and its aggregate owner budget. A source string containing a newline
is appended as content followed by the print newline; the whole append is
atomic for the budget. The native and tape liveness passes retain the JSON
root across a later Tool wait when the print still needs it; cancellation
drops it. An accessed missing/wrong-type/invalid string remains a task error.
The 64 KiB JSON token bound and 16 KiB default stdout bound remain distinct.

Targeted macOS tests covered ordinary, escaped, UTF-8/multibyte, empty,
duplicate-key-first, missing/null/invalid UTF-8, exact/over stdout, prior
output preservation, denied print, alias across a Tool wait and cancellation,
on both backends. Existing `string_contains`, `tool_json`, `http_tool`, and
default `scripts`/`interpreter` suites passed. This does not admit any of BN's
unchanged private `read_file` scripts: their original Host call spelling,
string argument and dictionary contract remain outside the current ABI.

Bounded resident-process source-cold full-request control:
`python3 tools/bo_request_pair.py`, raw
`evidence/bo-json-print/request-cost.jsonl` (1240 JSONL rows, SHA256
`44f7d89d5a484b7faabca736baa11f2f4503f4da9bb78670e09e5d1103c50fad`).
One fixture setup per process is reported separately, with 256 frozen Quotes
and an already resident JSON Bytes reply; each measured request includes
fresh source preparation, real ready Provider dispatch, print and task
cleanup. The old B binary is built from `0b6c826` with the *same* harness
overlay; A is the BO working tree. The script builds under the shared target
lock, checks every output, runs B/A/A/B in four independent processes, and
excludes the first request in each case/backend/process as warmup.
Toolchain: rustc 1.96.0 (`ac68faa20`), aarch64-apple-darwin; per-arm
release binary SHA256 is included in every raw row. The harness was overlaid
only onto B for a matching measurement, not committed into the B revision.
The invocation is limited to this two-pair campaign. A's new
natural string case median: tape 14.083 µs (160 requests), native 102.812 µs
(80). Old scalar A/B medians: tape 11.521/13.375 µs, native
98.980/106.334 µs; Quote A/B: tape 11.979/10.500 µs, native
106.626/95.791 µs. Process medians drift substantially across the run
(for scalar tape A 15.209→4.917 µs and B 20.917→3.646 µs), so these
aggregate deltas do **not** establish either a stable old-path regression
or a speed gain. The case is synthetic: no real filesystem Host adapter,
network latency, unchanged private script, Linux or Wasm claim.

## BP: BO mainline integration (2026-10-15)

Cherry-picked the reviewed BO commit `5003385` onto main `0b6c826`; no BN
runtime ABI or record-layout experiment was brought in. The 1240-row BO
resident-request raw file independently matches its documented SHA256 above;
its process-order drift does not establish a stable gain or regression. No
new performance campaign was run. README now describes the opt-in JSON print
behavior, and the separate [literal-string Tool argument proposal](string-tool-argument-proposal.md)
is design only, not an implemented capability or admission claim.

Local macOS aarch64 checks with shared Cargo target under the build lock:
`cargo +1.96.0 test --offline --locked` and the same with `--features tool-json`
(including the four `string_contains` tests exercising native and tape),
`cargo +1.96.0 clippy --offline --locked --all-targets -- -D warnings`,
the same clippy with `--features tool-json`, and `cargo fmt --all -- --check`
all passed. Rust 1.96's rustfmt component is absent locally; the available
default rustfmt was used without downloading a toolchain. Linux, Wasm and
production Host integration remain untested by this mainline integration.

## BQ: unified bound string Tool argument (2026-10-15)

The opt-in `tool-json` lane now binds a single named string argument through
`ToolDeclaration` (`ArgumentSchema::String`), in the same validated ID/name
namespace as numeric declarations. The existing `ToolSchema`,
`compile_with_tools`/`prepare_with_tools`, `Provider::call_tool`, `HttpProvider`
and numeric `[i64; 2]` fields remain source-compatible. The unified
`Provider::call_bound_tool` defaults to forwarding numeric calls and denying
strings; declaration, result shape and ID alone do not grant authority.
The Host-owned provider must authorize the ID and validate the path/reply,
including task-relative policy, redaction and bounds. No filesystem Host was
integrated. No claim that original private `read_file(...)` scripts run
unchanged: only static `tools.read_file("literal")` and previously declared
JSON string leaves/aliases are accepted; offset/limit, dynamic paths, lists,
imports and global aliasing remain unsupported.

Literal UTF-8 is bounded to 4,096 bytes; the whole source remains 16 KiB.
Sealed calls store an immutable `Arc<str>` or a JSON path slot. At dispatch,
an accessed plain leaf is validated then lends a `Bytes` span, while an
escaped leaf shares its decoded `Arc<String>` owner. The detached future owns
the span or decoded string after cancellation; the task drops unrelated JSON
owners at last use. Native uses the otherwise unused string-call `batch`
field to retain the post-capture liveness mask without growing `Frame`; tape
captures before releasing the source root. Both apply the pre-existing Tool
count/permission gate and JSON owner/stdout byte caps; errors preserve prior
printed output. A trusted Host must still bound concurrently detached work.

`tests/string_tool.rs` exercises literal and named binding, multiple declared
IDs, denial, missing/null/invalid UTF-8, Unicode print, stdout budget with
prefix preservation, schema/shape errors, old numeric API, and delayed
cancelled Tool completion with weak owner observation for escaped and plain
JSON on both native and tape. The response is a synthetic bounded JSON dict,
not a real `read_file` Host adapter. Existing `tool_json`, `http_tool` and
default feature tests remain the numeric regression control.

Small resident-process source-cold fixture: `cargo +1.96.0 test --release
--offline --locked --features tool-json --test string_tool
resident_request_cost -- --ignored --nocapture`. Raw
`evidence/bq-string-tool/request-cost.jsonl` has 104 rows, SHA256
`33ba921142e3e11d6c305aca97761519db18fe54f72453ab20f616827cd874eb`.
Source identity: this BQ commit on parent `bbca816`, benchmark harness
`tests/string_tool.rs` SHA256
`233495c8df46b385d99531bc5bcdda147415b95d87b49f0c6b5bcd6e85979e88`.
macOS 26.4.1 arm64, rustc 1.96.0 (`ac68faa20`); 26 fresh-source requests per
backend/argument shape, two initial warmups per shape excluded; alternating
numeric/string order. Timing includes source preparation, real dispatch of an
already-resident bounded JSON reply, stdout and cleanup; fixture/schema setup
and Cargo build are outside the interval. Median of 24 measured requests:
tape numeric/string 28.750/30.375 µs; native 172.416/170.333 µs. P90:
tape 49.666/44.958 µs; native 181.125/188.542 µs. Numeric vs string
inputs differ in argument semantics; these small synthetic, noisy paired
samples show neither a general speed gain nor a proven old-path regression.
An earlier same-command run varied materially in absolute medians (tape
17.750/17.375 µs; native 141.083/140.042 µs), reinforcing this limit.
No Monty baseline, unchanged private script, Linux/Wasm or production Host
validation is claimed by this bounded campaign.

## BR: bounded source-string sink on BQ (2026-10-15)

Cherry-picked BQ `6b48ebc` onto BP `bbca816` before extending the optional
`tool-json` lane. The sealed `StringSource` plan now holds up to 32 literal,
declared JSON leaf, or formatted scalar parts. Explicit `+` admits only string
literal/declared JSON parts; `f"...{n}..."` admits i64/bool locals/constants and
declared string leaves without conversions or format specs. The ordinary
scalar/numeric Tool path is unchanged. Original `read_file(...)` spelling,
string-valued local assignment, dynamic iteration, and a real filesystem Host
remain out of scope. This does not admit BN's unchanged private scripts.

Print validates all accessed parts before writing, checks the aggregate UTF-8
bytes plus newline once, and appends all parts atomically under stdout budget.
There is no joined print buffer. A joined Tool argument is materialized once,
bounded to 64 KiB and the remaining aggregate JSON buffer budget before
authorization/spawn; the detached future owns it through completion. Literal
pieces are individually limited to 4,096 bytes, source to 16 KiB, and the
existing 64 KiB per JSON string token, scan budget and Provider authority
remain. Native spills only referenced scalar locals and retains referenced
JSON roots across waits; tape tracks the same roots. Both share value selection,
error and output code. The static `StringSource::Joined` plan is not a general
Python string value or concatenation operator outside these consumers.

`tests/string_tool.rs` exercises literal and JSON composition, escaped Unicode,
two-root and scalar liveness across Tool waits, output prefix preservation,
aggregate join over-budget (no second Provider call), joined-argument lifetime
through cancellation, format admission errors, and both native/tape outputs.
Local macOS arm64 Rust 1.96.0 checks: offline locked
default and `tool-json` full test suites, both all-target clippy `-D warnings`,
and rustfmt. The installed `wasm32-wasip1` target is not available to the pinned
1.96 toolchain; cross-check failed at missing `std`, not project compilation.
Linux/Wasm execution and real Host integration remain unverified.

Synthetic source-cold resident-process evidence, with preparation, real ready
Provider dispatch, output and cleanup included; fixture setup and build excluded:
`cargo +1.96.0 test --release --offline --locked --features tool-json --test
string_tool resident_request_cost -- --ignored --nocapture`, plus the same for
`resident_sink_cost`. Raw `evidence/br-string-sink/request-cost.jsonl` (104 rows,
SHA256 `71ba9dcbc244c383632175901dbb207f24488dbccbe8f02256db4c0f274875b5`)
and `sink-cost.jsonl` (52 rows, SHA256
`d3753a6abb4476f49978ee2ab3259baf202546dc7cdaded78404445079bb3b76`).
The final harness `tests/string_tool.rs` SHA256 is
`99255c588ed7120702089b2c27ad7bb2a12dc8c579a5bdcf6c493f8403195b20`;
the benchmark measurements preceded test-only cancellation coverage/formatting,
not changes to the timed fixture or implementation.
Each backend/shape has 26 requests with first two warmups excluded. Current
numeric/string medians: tape 37.688/33.042 µs, native 179.188/174.855 µs;
joined sink (`"report" + ".txt"`, JSON alias, f-string print) tape 14.313 µs,
native 73.833 µs. Different source lengths/operations and process-order drift
make these shape costs, not an isolated sink overhead. A preceding run on this
candidate measured joined sink at tape 57.105/native 241.188 µs while its
numeric control was 33.917/175.958 µs. Therefore neither the apparent sink
speed nor old-path regression/improvement is established. BQ's earlier numeric
medians 28.750/172.416 µs were in another process/run. No Monty comparison for this synthetic Host-specific
call: this is not an unchanged private script or production workload.

## BS: string-script integration gate (2026-10-15)

Fast-forwarded BR `05773fc` onto BP main `bbca816` in the BS branch; BR includes
the BQ commit as `240a679`. Independent gpt-6-luna read-only review of that fixed
snapshot examined native/tape source-root liveness, authorization and output.
Its initial native dead-root concern was a false positive: native `suspend`
releases to the *pre-capture* mask, then dispatch captures `StringValue` and
releases to `after_json`; tape's `state.pc` has advanced to the following op,
so `json_live[state.pc]` is also post-capture. A second independent targeted
review confirmed the native correction. A tight 5,000-byte aggregate budget
test with two real 3,900-byte file replies confirms both engines release the
first dead root before installing the second. Existing BR tests cover live
alias across an intervening call, ordered errors, atomic stdout, cancellation
and late-completion owner; no production Host or platform claim follows.

`tests/local_read_host.rs` uses an actual temporary read-only filesystem Host
adapter and executes complete source on both backends: literal read→JSON
print, JSON content as the next path, prefix/f-string print, i64 f-string
argument, escaped UTF-8, denied absolute/traversal/symlink escape, missing,
invalid UTF-8, oversized reply and preserved earlier stdout. The adapter
canonicalizes under a fixture root and enforces a 4,096-byte file bound; it
is **not** Hermes's `read_file`, which also implements task-relative protected
paths, pagination, redaction and deduplication. The test fixture is trusted
and not a race-safe production filesystem sandbox. Historical private 0/64
complete-source admission, measured without Host schema, is unchanged and
cannot be translated into a coverage gain from these constructed scripts.

Bounded macOS arm64 Rust 1.96.0 resident-process new-source campaign:
`python3 tools/bs_request_pair.py` (shared fcntl build/measurement lock, one
identical harness overlaid into detached BQ `6b48ebc`, BS candidate BR base
`05773fc`). `evidence/bs-string-integration/request-cost.jsonl`: **936 rows**,
SHA256 `ddcde8f340d0f65be9ccb36d94736bcb32fd6650ccf331a1a4239f21432f5618`.
BQ/BS binaries and harness digests are in each row. Four process arms B/A/A/B,
24 requests plus one warmup per case/backend/process; fixture setup and build
separate, each timed request includes source preparation, dispatch of a ready
already-resident JSON reply, output and cleanup. Excluding setup (`index=0`)
and the first request warmup (`index=1`), pooled median BS/BQ old scalar tape
**14.917/6.875 µs (+117.0%)**, native **101.709/79.292 µs (+28.3%)**;
Quote tape **12.209/9.480 µs (+28.8%)**, native **106.771/85.667 µs (+24.6%)**.
These pooled differences are substantial but process drift prevents attributing
them as stable code regressions: scalar tape process medians B0/A1/A2/B3 =
**8.48/27.08/5.42/4.94 µs**, native **85.08/158.13/54.42/51.50 µs**.
The natural plain-string case BS/BQ pooled tape **13.333/11.958 µs (+11.5%)**,
native **88.833/76.041 µs (+16.8%)**; A2/B3 plain native
**63.08/65.58 µs** reverses sign. The pooled old-path slowdown is an observed
signal, not proof of its stable magnitude or absence: this bounded campaign
does not resolve a stable major old-path loss or a meaningful benefit.
Reproduce all BAAB aggregates from the immutable raw file (µs); each group
contains 48 measured requests, with 24 per process/case/backend:

```sh
python3 - <<'PY'
import collections, json, statistics
from pathlib import Path
r = [json.loads(line) for line in Path('evidence/bs-string-integration/request-cost.jsonl').open()]
assert len(r) == 936
groups = collections.defaultdict(list)
processes = collections.defaultdict(list)
for row in r:
    if row['kind'] != 'request' or row['index'] <= 1:
        continue
    us = row['ns'] / 1000
    groups[row['arm'], row['case'], row['backend']].append(us)
    processes[row['process'], row['case'], row['backend']].append(us)
assert len(groups) == 18 and all(len(v) == 48 for v in groups.values())
assert len(processes) == 36 and all(len(v) == 24 for v in processes.values())
for case in ('scalar', 'quote', 'plain', 'string'):
    for backend in ('tape', 'native'):
        a, b = (statistics.median(groups[arm, case, backend]) for arm in 'AB')
        print(case, backend, 'BS/BQ µs:', a, b, 'delta %:', 100 * (a / b - 1))
for case in ('scalar', 'plain'):
    for backend in ('tape', 'native'):
        print(case, backend, 'B0/A1/A2/B3 µs:',
              [statistics.median(processes[i, case, backend]) for i in range(4)])
PY
```
The sink diagnostic and consecutive-concatenation reference below use their
own `warmup` field, not request indices; their stated medians exclude it.
The synthetic string fixture is not actual filesystem I/O or a Monty
comparison; different semantics prevent claiming scalar/Quote speed ratios as
string speedup.

Independent allocation/wait diagnostic: `python3 tools/bs_sink_diagnostic.py`,
`evidence/bs-string-integration/sink-diagnostic.jsonl` (**208 rows**, SHA256
`c10ff9c48add2ba7c6ef5777a895b38aae65b64e5f2f02ae8cddc84147e2604a`).
Same output bytes for single f-string sink and multi-piece `+` sink; the
former has literal path, latter joins path pieces, so preparation is *not*
an isolated print comparison. Short `héllo` and long 4,096-byte UTF-8 content,
ready vs one-millisecond gated Provider; 12 measured new-source requests per
shape after one warmup. Counting global allocator reports *requested*
allocation bytes and calls inside this single-threaded process, not resident
memory or exact payload copies. On ready tape, short single/multi execution
median **8.8/7.8 µs**, long **20.3/19.6 µs**; native short **17.9/14.8 µs**,
long **29.2/25.9 µs**. In the long case, execution alloc calls are tape
**11/12**, native **12/13**; requested bytes **9280/9289** and
**9600/9609**, consistent with a small additional joined argument, while
preparation allocates many more objects and varies strongly by source form.
Gated execution is around **2.53–2.58 ms** in both paths due to timer/wait,
overwhelming the ready-path differences; it is not a service latency SLO.
Owner survival through cancelled gated operations is asserted separately in
`tests/string_tool.rs`, not inferred from allocation counts. A simple
same-output consecutive `String` concatenation reference parses the identical
JSON bytes but omits Host authorization, task limits, scheduling and source
preparation: `evidence/bs-string-integration/concat-reference.jsonl`
(50 rows, SHA256 `324cb3dc3f88a9af84c0e642fddad1b9cc718792e9d8e505e1ef823d4ecaeeea`),
short/long median **0.125/2.688 µs**. It is only a low-work floor, not an
equivalent full-request competitor. No general speed win is claimed.

Decision: retain the optional unified BQ declaration and bounded BR sinks
for the newly tested complete-source behavior; do not expand public semantics
or optimize noisy microdeltas. Default and `tool-json` full suites, all-target
clippy `-D warnings` in both feature modes, rustfmt and diff check passed on
macOS. Linux, pinned-toolchain Wasm and unchanged private-script execution
remain unverified; no cross-platform pass is asserted.

Post-integration review correction: an independent read-only Codex CLI pass
on signed `73130f6` identified a tape-only gap missed by the earlier BR review.
Before BQ, tape released dead JSON owners at every boundary. BQ deferred
release so a string Tool could capture its source, but restored early release
only for Tool, not `load_quotes`/`record_total`/yield. A dead 3,900-byte JSON
response plus a 4,096-byte Quote owner therefore falsely exceeded a 5,000-byte
aggregate budget on tape while native succeeded. The follow-up restores
pre-boundary release everywhere except Tool; Tool still captures before
releasing, and post-reply release remains. The regression
`dead_json_owner_released_before_quote_load_budget` executes both backends
with that tight real-file/Quote fixture. The independent CLI pass timed out
before its full report; this specific finding was traced and tested locally.
The previously published BAAB/diagnostic data belongs to `73130f6`, not the
follow-up; no performance delta is claimed for this correctness repair.

## String consumer and declaration allocation repair

Base `7f85049`. Declaration validation now borrows the existing schema rather
than deep-copying it into the numeric compatibility representation. Private
synchronous string sinks format scalar pieces on a 20-byte stack buffer and
use validated byte lengths before the final checked string access. Public
StringValue, errors, budgets, cancellation and per-task state are unchanged.

The isolated fixture records declaration validation **12→4 allocations (−66.7%)**
and eight scalar pieces **11→3 execution allocations (−72.7%)**. Both regression
tests fail on the base and pass on the candidate. These are allocation counts,
not whole-request speedups. The 4,848-row exploratory and 9,696-row confirmation
campaigns include resident-service source preparation and execution. Confirmation
supports lower tape costs for declared scalar and JSON output, while native
results and the first campaign show drift; no stable native speedup is claimed.

Keep these bounded work eliminations. Default/tool-json full suites, strict
tool-json Clippy, formatting and focused independent review passed. Patches,
raw, per-process reduction, timing boundaries and reproduction commands:
[evidence/string-consumer-fix](../evidence/string-consumer-fix/README.md).
No full benchmark or Linux/Wasm performance campaign was run.

## Bounded input-schema and HTTP collector experiments

Both candidates were withdrawn. Input-name pairwise comparison removes a temporary HashSet, but repeated long-prefix comparisons and drifting request measurements do not justify replacing the existing algorithm. Eight BAABABBA processes produced 4,848 raw records; see [input-schema evidence](../evidence/input-schema/README.md).

The HTTP singleton collector avoids one 128-byte container allocation but increases the concrete release fetch future by 40 bytes. A completed release ABBA loopback confirmation (2,000 calls) gives baseline/candidate medians-of-run-medians of 66,167/66,187.5 ns, with substantial variation. No speed gain established, and multi-chunk speed remains unmeasured. The initial incomplete pair is archived separately. See [HTTP evidence and reproduction](../evidence/http-single-chunk/README.md).

Production behavior remains unchanged. Keep the regression tests, test-only size probe, candidate patches and compressed raw; no new dependencies or worktrees.

## Reusable registered Tool environment

Add `ToolEnvironment::new(Vec<ToolDeclaration>)` and dual-backend
`prepare_with_environment` / `compile_with_environment` entries. Registration
validates and owns immutable declarations; each new source borrows that table.
Existing APIs keep their validation behavior and now also avoid copying the
whole table. Program-specific bindings still own their data, and Provider
permissions are checked at execution. Input schema validation remains per source.

A same-process, alternating-order probe records 2,624 rows over four paired
rounds. With 16 registered Tools and one ready call, tape preparation plus
execution improves **33.3–34.5%**, native **3.1–3.4%**, compared with passing raw
declarations on each request in the same build. One-Tool native cells remain
mixed. This does not isolate the additional old-to-new table-clone removal.
Default/tool-json suites and strict Clippy pass. Retain the opt-in API; no new
runtime dispatch or task state is added. See
[raw, reducer and boundaries](../evidence/tool-environment/README.md).

## BV: bounded input validation diagnosis gate (2026-10-16)

Base `7f1da48`. Keep production paths unchanged and defer BU's three mechanisms.
A private test-only diagnostic measures existing input validation, registered
frontend and full tape prepare/release in independent batch intervals. Across
two processes (32 batch rows), two-input validation averages have median
84.60 ns, range 54.17–207.38 ns; frontend 3.407 µs and complete tape preparation
3.567 µs. The intervals are not additive and do not prove an E2E saving.

The fixed formal binary A/A uses 0/2 inputs, scalar/ready cases and both
backends: 64 processes, 2,048 measured fresh-source requests, no failures.
Two-input tape scalar A/A B/A is 1.029, ready 0.994. Cold/process drift remains
substantial (zero-input tape scalar A/A 0.892). The isolated work is too small
and request-level value too uncertain to justify a new public schema API here.
No A/B implementation is claimed. The operation/guest single-job topology and
single-PC native entry remain deferred; no second candidate is manufactured.

Retain the opt-in two-input workbench cases, private diagnostic and compressed
raw. Existing validation, error ordering, effect detachment, owner/cancellation,
permissions, budgets, fairness and future layout remain identical. No allocation,
density, Monty, Linux or Wasm improvement is claimed. Commands, per-process raw,
timing boundaries, diagnostic limitations and decision ledger:
[BV input gate](../evidence/bv-input-gate/README.md).

## BW: synchronous joined string print borrowing

Base `c8d28f0`. Retain a private two-pass synchronous sink: validate/size in
original piece order, check authority/output budget and reserve, then borrow
prepared text directly into stdout. Literal/JSON owner clones and escaped-String
Arc promotion are eliminated here; asynchronous Tool captures remain owned.
The escaped same-output execution diagnostic removes one allocation on both
backends (joined 16→15, plain control stays 15). Output/SinkPart sizes remain
72/56 bytes and temporary slot storage remains 1,792 bytes; no density gain claimed.

Two-round fixed-binary ABBA and A/A each complete 112 processes / 5,376 measured
fresh-source requests with no failures. Full-request ratios are small and mixed,
with large early process drift even in untouched controls. No stable E2E speedup
is claimed. Keep the demonstrated work removal, stop timing here. Default and
tool-json release suites, strict tool-json all-target Clippy, atomic error-order
and detached cancellation regressions pass. No Linux/Wasm/Monty performance run.
Raw, candidate patch, diagnostic outputs, reducer and commands:
[BW sink evidence](../evidence/bw-sink/README.md). The pre-created BX card owns the
next bounded generic checked multiply-add tape experiment.

The focused independent `gpt-6-luna` review of fixed integration commit
`b3de3e8` found no blockers and independently reproduced the evidence reduction.
The delegation receipt verified the requested model. An earlier standalone
Codex attempt failed before review because that account did not support the
model; no substitute was used. See [review record](../evidence/bw-sink/review.md).

## BX: checked multiply-add tape fusion rejected

Bounded local peephole experiment on `889b134` (macOS ARM64, Rust 1.96.0):
fold an immediately preceding temporary multiply consumed by Add into one
checked multiply-then-add instruction, with general bound operands and unchanged
source/error order. Default and tool-json/debug-runtime release suites pass,
including signed overflow, self-assignment, short-circuit and distinguishable
JSON-vs-overflow errors. Existing boundary/lifetime tests remain unchanged.

The certain cost is global Op stride growth from 56 to 64 bytes (+14.3%), including
unmatched code. Matched short prepared capacity falls 608→408 bytes at a Vec
threshold, but computed-loop capacity rises 1104→1224 bytes; task State (312 B),
unit (112 B) and synchronous scratch (4096 B) do not change. No waiting-state
density, allocation or RSS improvement is claimed. Preparation-only diagnostic
runs visibly warm differently and cannot establish a causal timing delta.

Fixed-binary A/B and A/A each contain 80 successful processes / 3840 measured
fresh-source full requests. Tape B:A: short multiply-add 0.9683, original Quote
0.9457, computed-operand loop 1.1786. Unmatched scalar/ready and unchanged native
controls also drift, with A/A up to 4.1%. There is no consistent matched-request
net win sufficient to justify global instruction widening. Remove the production
candidate rather than adding layout/storage machinery or extending the campaign.
Only regressions, a test-only diagnostic, adapter cases and evidence remain.
Source manifests now include nested Rust modules, including the tape source.

Small raw results, rejected patch, commands and reduction:
[BX evidence](../evidence/bx-madd/README.md). No Monty/Linux/Wasm performance claim.

The focused independent review of fixed integration commit `6c7b577`, with
receipt-verified `gpt-6-luna`, found no blockers and independently reproduced
the evidence reduction. Final restored-source default and tool-json/debug-runtime
release suites pass, as do strict all-target Clippy, 9 Python workbench tests,
rustfmt and patch/diff checks. Tests use Rust 1.96.0; installed Clippy/rustfmt use
Rust 1.93.1. See [BX review record](../evidence/bx-madd/review.md).

## BY: explicit Ready Tool dispatch deferred after bounded prototype

Base `03a39af`. An additive Provider begin_tool returning owned Ready(result) or
Deferred(arguments) can preserve existing Provider source compatibility; the
default remains detached. This isolated prototype never polls a future inline.
Ready completion catches unwind panic, yields once and rechecks cancellation;
Deferred retains the original spawn/oneshot ownership, HTTP permits and timeout.
Both engines preserve argument errors, authority, call debit and reply consumers.

A/B and exact-binary A/A each complete 64 processes / 3072 fresh-source requests.
Ready tape B:A is 0.9470, native 1.0106; unchanged scalar tape is 0.9433 and A/A
controls drift up to 2.32%. Thus there is no consistent causal request win.
Separate prepared-execution diagnostics remove two allocations / 480 requested
bytes on explicit-ready calls, but concrete execution future sizes increase
tape 928→1144 B and native 664→880 B, including Deferred mode. No density/RSS or
HTTP speed gain is claimed. Preserve fairness rather than removing its yield to
rescue the measurement. Stop this candidate and restore all production/API and
normal workbench code exactly to the base; keep patch, tests and small evidence.

Candidate and final full default and tool-json/debug-runtime release suites pass
on Rust 1.96.0. Initial 64-call fairness test exceeded existing MAX_LOCALS; its
passing eight-call replacement does not relax admission. Final strict all-target
Clippy (installed Rust 1.93.1), nine Python workbench tests, fmt and patch/diff
checks pass. No Linux/Wasm/Monty performance campaign. The pre-created BZ lane
owns the distinct large-root/small-leaf async retention experiment.
Commands, raw, layout/allocation limits and compatible API prototype:
[BY Ready evidence](../evidence/by-ready/README.md).

Receipt-verified `gpt-6-luna` independently reviewed fixed snapshot `3afda8f`,
confirmed the prototype stays archived and final production matches the base,
recomputed timing/layout evidence and found no blockers. See
[BY review record](../evidence/by-ready/review.md).

## BZ: large-root / small async parameter lifetime, bounded stage decision

Base `f311832`. A private candidate copies only an already validated direct plain
JSON leaf when its root has no continuation consumer, root length >=64 KiB,
leaf <=4096 B and current transient budget has headroom. These are experimental
engineering cutoffs, not a wait predictor or measured optimum. Existing
validation/scan fuel, authority order, call debit, post-capture root release and
detached operation cancellation/ownership remain. Escaped/concatenated strings
already own their text. Low headroom falls back without introducing a task error.

Separate Box-backed Bytes diagnostics explicitly deallocate the backing before
publishing a release marker: both backends release 1,048,621 B before waiting and
retain a 10 B copied parameter, still readable after guest cancellation. Live
roots, small roots, long leaves and tight-budget controls keep the old owner;
escaped strings already release it on baseline. Wrapper future remains 984 B,
not a per-backend future measurement. Preliminary Weak<Arc<[u8]>> observations
are retained but excluded from backing-deallocation claims; the Weak retains its
allocation. Corrected tests and diagnostics were rerun. No RSS/census claim.

Formal A/B and exact-binary A/A each completed 64 processes / 2048 fresh-source
requests in two ABBA rounds, with all status/stdout assertions passing. Detach
B:A is tape 1.01754, native 1.00914, with early process drift and untouched scalar
control movement. This establishes no stable request gain or causal regression.
Retain the mechanism as an isolated patch, defer the default copy policy and
restore production exactly to base. The ready synthetic workload establishes
neither real waiting distributions nor prevalence of this retention opportunity.

Stage: BV defer, BW keep measured allocation elimination without E2E claim,
BX revert widening fusion, BY revert its every-Ready-yield representation (not
an amortized allowance experiment), BZ defer workload-dependent copying despite
proven backing release. Stop this stage rather than chase a speedup. The
pre-created CB lane tests a distinct larger hypothesis: remove per-source native
backend work with actual precompiled templates, bounded checked pure regions,
installation/cleanup counted and current unfused tape as the control. No C&P
result is claimed here. Full commands, corrected tests, raw, synthesis and
reducer: [BZ evidence](../evidence/bz-detach/README.md).

Receipt-verified `gpt-6-luna` independently reviewed fixed snapshot `24518d5`,
confirmed production restoration and backing/cancellation reasoning, reproduced
the reduction and found no blockers. See [BZ review](../evidence/bz-detach/review.md).

## CC: accept bounded waiting-memory policy after concurrent census

Reassess BZ for waiting memory rather than require a speedup. Integrate its private
helper in both direct JSON Tool captures: validated plain leaf <=4096 B, logical
root >=64 KiB, root dead in continuation, transient headroom sufficient. Low budget
falls back to the old owner without a new error. Original scan charging,
authorization/call debit, post-capture release and detached cancellation remain.
Extra payload/copy work is bounded to 4096 B per capture; no new frame/state/API.

One gated concurrent fixture, baseline and accepted source, runs six modes on
both backends, eight simultaneous blocked operations each (96 tasks per arm).
For dead roots each backend deallocates 8,388,968 B of actual Box backing before
wait observation and retains 80 B of copied arguments. Small/live roots,
4097 B leaves and tight budgets keep original owners; escaped strings already
detach on baseline. Half the guests cancel while operations remain blocked,
then every operation correctly reads its argument and releases it on completion.
Other guests succeed with exact continuation output. This proves backing lifetime
for unique test owners, not returned OS pages, RSS, wait prevalence or throughput.

Full default/tool-json-debug-runtime release tests, exact cutoff/headroom/scan
unit coverage, strict Clippy, fmt and diff checks pass. Existing external shared
target and fcntl lock, one build job, offline/locked Rust 1.96 tests; installed
1.93.1 Clippy/fmt. Reuse BZ latency evidence with no new campaign or speed claim.
Accept the small bounded policy for its large proven waiting-owner benefit;
threshold optimality and real workload frequency remain unknown. Full decision,
raw, reducer and reproduction: [CC evidence](../evidence/cc-waiting/README.md).

Stage: BV defer, BW keep allocation removal, BX/BY reject measured implementations
(compact fusion/amortized Ready remain untested), BZ/CC keep bounded wait-memory
release. CB remains isolated on `exp/cb-copy-patch` at `814fdd8`: real templates
and direct installation work, but its optimistic raw install/call/free floor
3.989 us already consumes the tiny full tape request 3.542 us. Stop that tiny-glue,
per-request JIT-module design only; source backend/loop matrix were not reached
and larger-loop value remains untested. No new worktree/target/download/CI or
Linux/Wasm/Monty performance claim. Stop the stage here.

Receipt-verified `gpt-6-luna` independently reviewed fixed commit `29077ad`,
replayed the census reducer, verified all 22 final source hashes against commit
blobs and found no blockers. The raw records pre-commit base HEAD plus actual
source hashes; this provenance distinction and baseline replay are clarified in
the following documentation-only update. See [CC review](../evidence/cc-waiting/review.md).

## CD: remove acyclic tape convergence confirmation

Base `0e2caed`, macOS ARM64/Rust 1.96. Test-only instruction visit counters
confirm the supplied CA hypothesis: nonzero buffer/JSON masks on forward-only
tape took two reverse passes. Identify backward/self edges in the existing
transfer arms and end after the first pass only when none exist. Preserve the
original fixed-point solver for loops. No CFG, prepass, persistent metadata,
allocation, API, runtime check or task layout change. Native scalar/owner
analysis remains unchanged; tape scalar slots have no liveness analysis.

The pre-CD oracle matches every PC mask across branches, multiple owners,
Tool string captures and loops. Nonzero DAG analysis visits halve, e.g.
buffer branch 20→10 and JSON string capture 14→7. Loop examples stay 36→36;
all-zero buffer and absent-JSON paths already did no redundant work. Existing
native/tape differential, error-order, wait/cancellation and CC retention tests
pass with full default and tool-json/debug-runtime release suites, strict
Clippy, fmt, diff checks and nine workbench tests. Initial unsupported-return
test fixtures and a missing opt-in case-list expectation were corrected without
relaxing admission or assertions; verification history is retained.

One fixed-binary A/B and exact-A A/A campaign each completes 80 processes /
3,840 fresh-source prepare+execute+cleanup requests with exact output/status.
Tape B:A scalar .97699, branch .98455, ready JSON .95214, branch JSON .96708,
Quote loop 1.00468. A/A and untouched native controls drift too. There is no
meaningful observed request regression and no causal speedup claim. Keep the
certain small work removal and stop measuring. CC and isolated CB stay intact;
no new target/worktree/dependency/download/CI or Linux/Wasm/Monty campaign.

Raw identities, source-hash provenance, visit counts, commands, verification
and bounded decision: [CD evidence](../evidence/cd-acyclic/README.md). Next
unresolved question is the dominant preparation cost in representative real
Host sources and loop/wait mix, before selecting further representation work.

Receipt-verified `gpt-6-luna` independently reviewed fixed commit `528f7fc`,
replayed both evidence reductions, checked all 24 B source hashes against Git
blobs and found no blockers. Lead separately verified those source hashes and
the unchanged CB branch. See [CD review](../evidence/cd-acyclic/review.md).

## CE: coalesce pure native arithmetic overflow exits

One bounded candidate on `e3055b2`, macOS ARM64/Rust 1.96. Cranelift's actual
lowering retains per-operation overflow branches. For a single pure add/sub/mul
tree with infallible scalar/validated-buffer leaves, retain all intermediate
flags and branch on their OR before committing the result. Preserve tree and
left-to-right order, short circuit and all effect/error boundaries. Overflow
has one existing error identity without a source position. No reassociation,
tape/Op/ABI/runtime/resource-policy change, persistent metadata or dependency.

Separate actual CLIF/VCode diagnostics show four overflow exits reduced to one,
short input-bound code 228→212 B and computed-loop code 600→528 B. This removes
real control work, adding three flag ORs. Tiny BT request literals can constant
fold; do not equate its timing with the four-operation input-bound diagnostic.
Full native/tape suites cover intermediate overflow that later returns in range
or multiplies by zero, prior stdout, skipped RHS, aliasing, JSON error order and
prevented later effects. Final default/tool-json/debug-runtime release suites,
strict all-target Clippy, fmt, nine BT Python tests and diff checks pass.

Fixed-source-manifest A/B and exact-binary A/A each contain 80 successful
processes / 3,840 complete prepare+run+cleanup requests. Native tiny/compute-loop
B:A are .94332/.94984, their A/A .99696/.99806. Original Quote .99565 versus
A/A .98357 has no established benefit. Native Ready control also drifts to
.96048 and one candidate tiny process is slower, so no precise causal 5% or
general-workload speedup claim. Keep the modest local candidate based on certain
code/work reduction and bounded matched-request support; stop without scaling
samples. No Linux/Wasm/Monty, RSS, allocation, density or throughput claim.

CC/CD and isolated CB remain intact. Existing external target, one build job,
offline/locked test builds and common fcntl lock were used. Environment deviation:
`cargo +1.93.1 fmt` auto-installed that named toolchain although existing stable
already reported 1.93.1, violating this card's no-download constraint. Default
toolchain/config was not changed; subsequent commands use installed stable and
disable auto-install. First Clippy's collapsible-if finding was fixed normally;
no suppression or weakened tests. Final CLIF/VCode equal the measured candidate.

Commands, small raw, exact reconstructed source identities and verification:
[CE evidence](../evidence/ce-overflow/README.md).

Receipt-verified `gpt-6-luna` independently reviewed fixed commit `7789bbe`,
replayed reductions and source hashes, and recommends keep without blockers.
Its fixed-range diff check found four logs' trailing empty lines missed by the
lead's working-tree-only check. Text-only follow-up removes them and verifies
the whole base-to-result range. See [CE review](../evidence/ce-overflow/review.md).

## CF：近期优化组合验收与阶段结束

固定 pre-BW `c8d28f0` 和 main `1507ae8`，验证祖先关系和最终源码差异，
只保留 BW/CC/CD/CE 的组合；没有生产代码修改。四类 identical-adapter
新源请求：短 branch、escaped joined print、约 1 MiB JSON 的小 string
参数经真实 Notify-gated Tool 等待、256-row computed arithmetic loop。
双后端 A/B 与 exact-A A/A 各两轮 ABBA、64 进程 / 1536 正式请求，所有
status/stdout/等待阶段断言通过。计时包括 prepare、执行、生产者构造导入、
code/output 和 gate 清理，不含调用方 source 构造与 resident fixture 启动。

tape 的 branch/join/wait/loop B:A 为 .89676/.97825/.99288/1.01372；native
为 1.01051/.98591/1.01577/.92223。早期进程漂移显著，branch 全零 owner
mask 也没有 CE 多算术树，不能将其差异归为 CD 收益。native loop 的局部
支持不扩展为精确因果 7.8% 或通用加速。其余小幅变化混合，不追加样本。
CC 继续以等待 backing 释放为保留依据，不因未证延迟改善自动回滚。

分别保留已证实的 BW 一次 allocation 去除、CC 每后端八 owner 的
8,388,968 B backing 释放 / 80 B 参数、CD 非零 DAG visits 减半、CE
四到一 overflow exits。复用旧 CC census 并在当前 main 重跑原 fixture，
不将 backing 重新命名为 RSS。完整 default/tool-json-debug-runtime release
suites、严格 all-target Clippy、fmt、九个 Python tests 通过。正式两臂源码
hash 对各自 Git blob 完整校验，包含显式相同 CF adapter / inert test overlay；
采集时 A/B 的 HEAD 标签均为 main，源码 hash 才标识临时 baseline 内容。

先枚举 toolchains、禁用 auto-install，再在 no overrides、进程/open-file
与项目引用检查、49,691 个文件与 stable 对比后，只删除 CE 误装的命名版
1.93.1。所有程序/库与 stable 相同，仅两项 rustup inventories 不同。
stable（仍 1.93.1）、1.92/1.96、共享 target 和其他项目保持。无新
worktree/target/dependency/download/CI 或 Linux/Wasm/Monty 性能结论。

阶段结束，保留 BW/CC/CD/CE。最高价值问题是实际 Host source 和 cold-prepare
比例：历史 BT 独立 instrumented Ready raw 的 backend/prepare 中位比例
78.7%，与本次 native/tape 总成本差距指向 preparation 的调查价值，但不是
当前 phase share 或新 backend 的批准。先判断真实 source/loop/wait 分布，
不自动继续零碎优化。小 raw、归约、边界、清理和复现：
[CF stage evidence](../evidence/cf-stage/README.md)。

独立 `gpt-6-luna` 对固定 `eea1a46` 聚焦审阅，主控回读 receipt 确认
精确 model/provider 和四次主推理调用。Reviewer 重放完整归约、确认 summary
不变并检查固定范围 diff，无阻塞项，不追加构建或计时。
见 [CF review](../evidence/cf-stage/review.md)。

## CG：真实 Code Mode 适配与下一片段决策

研究结论：暂缓新实现，不继续数值循环/dispatch 微优化，不创建重复下游卡。
复用 BI/BK 原 64/520 CLI 完整源码与 Z cutoff/replay 规则，重新核对 CLI
会话、replay 和完整 aggregate，两次纯 AST 分析输出完全一致。没有语料
执行、真实 Tool 调用、Cargo 构建或新 prepare 实验，不重复旧无 schema 0/64。

源码 UTF-8 min/median/p90/max 为 230/626/1163/2019 B，行数为 1/4/11/22。
61/64 有 terminal 或既有外部 shell helper 名称调用，0/64 使用当前
`tools.<declared>` 拼写；这是 Python Code Mode 内的通用命令编排形状，
不是混入了终端 Python。11 个脚本有显式循环，20 有推导式/生成器，并集
25；3 个脚本的循环 AST 内含已识别 Host/shell 调用。它们不证明数值热循环、
实际 effect 次数、CPU 或 wait duration。原 520 个去 replay 的提交源码全部
不同，本 64 个各出现一次；未观察到源码重提，不等于已测 production cache
命中率、prepared object 复用或进程冷启动比例。

当前 64 个完整源码均有有限 builtin/Tool 语法以外的 bare-name call；
字符串 sink/静态 JSON 叶子并未解决这个绑定缺口。read helper 的 15 个中
13 个还含 shell helper，余下两个还有路径存在性/条件表达式或 import/
推导式/对象操作。新确认启用的原样完整脚本 0 个；本卡未改写执行脚本，
适配后的实际通过也未建立。以上为静态阻碍和交付计数，不是新 admission run。

唯一保留的设计场景是 BS 已存在的单只读文件 Tool→content→有界 print
fixture，无需重新建设。源码核对其 4096 B/固定可信 root/UTF-8/错误契约，
并重查本机真实 Hermes read_file 的分页、脱敏、任务路径与 dedup/error
多形返回；fixture 不等价于真实 Host，不能把缺失 content 静默转为空字符串
来制造兼容。现有通用执行器仍是简单替代；待一个真正有限 Host 完整契约和
完整程序成立，再决定性能调查，不批准新语言/后端/cache 架构。

脱敏报告、边界、聚合与重放身份见
[CG workload fit](../evidence/cg-workload-fit/README.md)。私有探针/语料不进仓库，
无新增依赖、下载、worktree、target、CI、平台执行或生产性能结论。

## CI：实际指令、布局与局部性诊断

以 `ebee3c1` 为基线，仅临时接入 test-only probe，最终生产源码逐字节
保持基线。Apple M4 / rustc 1.96.0、已挂载外置共享 target，全程持有
perf build lock。九类 bounded source 在 tape/native 两后端实际执行并
核对完整输出；另以 256/4096/65536 rows 分开验证循环 dispatch、fuel
reservation 和 yield 计数。没有新请求计时、PMU misses、RSS、Linux/
Wasm/Monty 或真实 Code Mode 支持结论。

实测 Op/Operand/Dest 为 56/16/16 B，Unit/State 为 112/312 B，同步
scratch 4096 B，native Frame 320 B，本卡 Provider 的完整 execute
future 为 tape/native 928/664 B。实际 release tape 反汇编保留 PC 写回、
opcode/operand 解码、间接 dispatch、bounds 和 checked-overflow 路径；
native Quote loop 的累加器保持在寄存器，CE 的共享错误出口仍在。
不能由布局大小推断 cache miss、RSS 或端到端加速。

CJ 唯一推荐候选：admission 已证明 `buffers==0` 时跳过 buffer-only
liveness allocation/逆序扫描，保留独立 JSON liveness 和非零 buffer
检查。七类 zero-buffer fixture 仍有 32–72 B live backing；预期每次
prepare 少一个 allocation 和一次 traversal，尚未实测候选净收益。
这比改编码更直接。只窄化 Dest 的完整镜像 Op 可实测为 48 B，但只列
第二候选，暂缓实现，不与 primary 捆绑。BT/BX/BY/CB 负结果及 CG 的
真实 workload 适配 stop 结论不变。固定 raw、源码重建、反汇编、排名、
CJ 单轮 A/B+A/A 契约和复现见 [CI evidence](../evidence/ci-layout/README.md)。

## CJ：省略 zero-buffer liveness

保留最小候选，结束本轮。Admission 的 `buffers==0` 直接跳过 buffer-only
Vec allocation/transfer pass，独立 JSON liveness 不变。统一 production/
debug mask 读点；非零 buffer 仍用 checked PC 索引，新增越界回归和逐 PC
oracle，zero case 明确断言零 backing/零 visits。

Apple M4 / Rust 1.96.0、既有外置共享 target 和 fcntl lock。预设七类双后端
A/B 与 exact-A A/A 各两轮 ABBA：224 进程、5,376 measured requests 全成功。
独立三次 prepare allocation census 确认七类各少一次 allocation、32–72 B
requested/backing bytes，两个非零 buffer 对照及九类 native prepare 不变。
Unit/State/future 和 Op 布局不变，实际 async dispatcher 多九指令/36 B，
新增 admitted-buffer 分支在 effect/yield boundary，不在逐 row dispatch。

延迟混合：tape branch +7.3%、quote +4.1%、madd_loop -4.4%；未变 native
也有约 -3.9% 至 +2.4% 差异，最早 scalar 进程明显漂移。没有追加样本，
不宣称通用提速或无回归界。保留依据仅是确定删除的 prepare work/backing，
报告新增边界判断成本，不把静态 bytes 改称 cache/RSS 收益。完整 default/
tool-json-debug-runtime release suites、严格 all-target Clippy、fmt、九个
Python workbench tests 通过。正式 source hashes 逐 Git blob+patch 重建。

没有新 worktree/target/dependency/download/CI 或 Linux/Wasm/Monty 结论。
第二布局候选不启用，BT/BX/BY/CB 负结果与 CG workload-fit stop 保持。
raw、源码身份、独立诊断、工程失败说明、复现及审阅见
[CJ evidence](../evidence/cj-zero-buffer/README.md)。

## CK：tape 临时槽初始化的单一安全候选

固定 `049b810`，验证每个 production 同步 turn 实际清零4096 B，scratch不跨
await；debug Step则必须保留表达式中间值。Lowering的既有硬上界512槽，独立
CFG definite-write oracle覆盖effect/可能yield的清空、短路/分支/循环/JSON，
实测0至127槽。唯一候选在lowering记录高水位，<=16槽用128 B安全数组，
否则原4096 B，共享slice dispatcher，无unsafe、新allocation或执行器复制。

正式A/B二进制反汇编确认small path四条向量pair stores替代4096 B `_bzero`；
大路径保持。但wrapper outlining令同步栈4256→4368 B，Unit112→120 B，
增加阈值分支和一次call；具体Provider等待future仍928/664 B。不能将清零
范围下降称为栈frame缩小，更无PMU/cache/RSS结论。

预设A/B、exact-A A/A各一轮ABBA，五类双后端，共80进程/1920正式完整请求
全通过。tape scalar/branch/madd_short/loop/wait B:A为
.97750/.99697/.96751/1.00873/1.00050，scalar A/A为.92598；native未变
也有漂移。独立gpt-6-luna审阅固定 `6e229fd` 后建议撤回，无正确性阻塞；
主控验证receipt为6次精确model/provider主推理调用，已撤回并逐字节恢复
基线生产源码/测试。Oracle未建模普通statement切换的清空，不能当作完整
语句间读前写证明；此限制已缩窄记录，候选内存安全不依赖它，不修补已撤回方案。
完整default和tool-json/debug-runtime release suites、严格Clippy/fmt、
九Python tests通过；新真实执行回归覆盖小/大scratch、深表达式、600行
chunk7以及Tool挂起。无新target/worktree/dependency/download/CI或平台结论。
原始证据、逐source重建、失败说明和复现见
[CK evidence](../evidence/ck-scratch/README.md)。

## CL：紧凑目的槽，以及 CI–CL 阶段结束

固定 `552faed`，只将私有 Dest Local/Temp 改为 u8/u16，lowering 以
checked conversion 构造，读取无损转 usize，原 admission 上界、临时槽
assertion、数组 bounds checks 和 debug destination tracing 保留。
Operand/i64、dispatch、scratch、liveness、权限/预算/效果合同不变。

Apple M4/Rust1.96.0 的真实 Dest16→4 B、Op56→48 B；九类 fixture
Op数量/capacity不变，各少32–128 B backing（14.29%）。Unit112 B、
State312 B、具体Provider future928/664 B、同步栈4256 B不变。正式
binary的实际tape dispatcher1907→1875指令（7628→7500 B），ldrb/ldrh
加载目的索引已做零扩展，未增加独立uxt指令，原bounds exits仍在。
不把静态bytes称为动态指令、PMU/cache miss、RSS或吞吐收益。

预设七类双后端A/B与exact-A A/A各一轮ABBA，112进程/2688正式完整
新源请求全通过。tape scalar +7.9%、branch −5.1%，其余约−.9%至+.4%；
首批短程序进程明显漂移，未变native也有−4.7%至+3.3%差异。全量保留，
不事后筛除、不追加样本；没有一致请求净收益或可信归因的回退结论。
按简单表示和确定backing密度保留，不宣称通用提速或无回归界。

新增真实512-node admission边界程序，477临时槽/最高476，双后端和debug
单步通过；32locals末槽、alias/branch、600rows/chunk7、unchanged-value
local写trace、额外node/local/input错误合同通过。独立sealed诊断对全部
512槽（含511）round-trip且验证非法索引仍panic，不冒充公开可达workload。
完整default与tool-json/debug-runtime release suites、两套严格all-target
Clippy、stable fmt、九Python tests通过。外盘共享target、fcntl锁、offline/
locked单job；无新target/worktree/依赖/下载/CI/webpage或平台结论。

结束CI–CL低层微调阶段：CJ保留确定prepare work/backing删除，CK撤回，
CL保留此最小密度改动。不同campaign不能相加为阶段加速；CG真实workload
适配stop及BT/BX/BY/CB负结果不变，不自动重开撤回方案或扩张缓存/编码/
后端/Tool适配。原始证据、源码重建、反汇编与复现见
[CL evidence](../evidence/cl-dest/README.md)。

## CM：CI–CL 主线集成

`main` 从 `ebee3c1dfd87b4835f8bc16a00c2734f3dd53d69` fast-forward 到
已审阅分支顶端 `2f75eb348a6139696d8404c9e1a391107ebf11e5`，随后只增加
本节和 roadmap 当前入口的签名文档提交。不 squash、不重写或删除实验分支。
合入树与固定 CL 审阅树相同，没有冲突解决或额外生产实现。

逐 Git diff 核对 `src/`、`tests/`、Cargo manifest/lock：`049b810..552faed`
为空，CK 已完整撤回；`53fcd0a..2f75eb3` 等于 `552faed..2d873b3`，
最终生产/测试差异恰为 CJ 加 CL。新增追踪路径仅为这些源码/测试、复现脚本、
CI–CL evidence 和实验文档；未带入 target、依赖树、formal binary 或凭据文件。
保留的 `.gz` 是既有压缩 raw/patch/反汇编证据，不是构建产物。

本卡复用同树验证，没有重新构建或计时。实际回读
`evidence/cl-dest/verification.json.gz` 的全部11条 exit0记录，逐 SHA-256
确认全部生产源码和 `tests/cl_dest.rs` 与当前树一致。既有证据包含完整
default 与 tool-json/debug-runtime release suites、两套严格 all-target
Clippy、stable fmt、九个 Python tests 和 sealed bounds 诊断。
本卡只读重放 `tools/cl_dest_reduce.py`，拦截 summary 写入并核对与已提交
summary 逐字节相同，重建 source manifests、核对完整 campaign 和布局不变量。
另运行九个 Python tests 和 `git diff --check`；未重新声称 Cargo suites 执行。

复用 [CJ 固定审阅](../evidence/cj-zero-buffer/review.md) 和
[CL 固定审阅](../evidence/cl-dest/review.md)，不启动重复审阅。
CJ 保留确定 prepare work/backing 删除，CK 撤回，CL 保留最小目的槽表示；
没有通用请求提速、无回归界或累计阶段加速结论。CG stop 与 BT/BX/BY/CB
负结果不变。此卡只完成主线整合，不开启新实验、CI、依赖下载或平台验证。

## CO：单一筛选计数标量批量 kernel

固定 `44212e0`，生产release/Cargo配置仍与 `907eb40` 相同，未启用PGO。
只匹配整个loop体为 `if row.FIELD > THRESHOLD: count=count+1` 的纯形状，
两个既有signed-i64字段，literal或非count local阈值。lowering以Start+Count
替换原tape区段，40B descriptor内联，Op48B、Unit112B、State312B及本具体
Provider tape/native future928/664B不变，无IR备份或新heap/执行器。
其他形状仍走原tape；debug-runtime保持原逐Op trace，不融合。

独立600行诊断：chunk256两字段dispatch2407→10，fuel3/yield2不变；chunk7
2407→93，fuel86/yield85不变。匹配形状tape11→7 ops、capacity16→8，
prepared逻辑976→560B；alias与普通求和对照不变。实际正式arm64机器码每个
kernel20指令/80B，scalar ldr/compare/checked adds/b.vs，没有SIMD。
不能只数被LLVM分离后的wrapper：dispatcher7500→8084B，selected tape
text+1116B，同步栈4256→4336B（+80），scratch仍4096B，future未变。

九类双后端、每cell12正式请求、一轮ABBA+exact-A A/A。首对未保存期间CPU
记录，降级保留；原binary/候选/计划不变，仅补一对带CPU记录替代采集，
不池化、不调参、不做第三轮。共288进程/3456正式请求，全部exit0且精确
status/stdout断言通过。替代对large price/quantity/none/all tape B/A为
.42109/.46453/.51505/.41193；小输入1.00063/.99487，对照scalar/quote/ready
.97786/1.00072/1.02991。原未控对小输入1.20180/1.09843及明显进程漂移
完整保留，不能用替代对覆盖回退或用A/A声称噪声界。startup独立记录。

替代对前两次有效top idle96.46%/97.14%；期间只有两snapshot，首次累计
snapshot不能当delta，后次87.74% idle，无记录到≥50%无关单核竞争者。
reader漏结EOF最后block，使采集器后置样本数断言失败，但两个raw均完整，
CPU原文仍保存；已补EOF结算但不重跑计时。低频记录不是整轮硬独占证明。
不从当前空闲反推过去干扰，不改CN结论，CR负责独立复核PGO。

最终完整default/tool-json/tool-json+debug release suites79/130/138通过，
三套严格Clippy、fmt、9个BT Python tests和diff check通过。覆盖字段/尾部/
signed extrema、overflow/effect顺序、fuel/index/yield、取消与弱owner释放、
JSON跨融合chunk保留、未匹配形状和immutable unit复用。early fixture与
记录脚本失败逐项保留。全部共享外盘target/锁/单job/offline，未新增依赖、
下载、worktree、CI、网站或平台执行。

取舍：保留窄候选在独立分支，停止CO；大输入完整请求和确定dispatch删除
支持继续评估，小输入/真实Host通用净收益未建立。main集成等待原卡审阅，
CP再判断SIMD净值，不先扩框架或启用PGO。源码hash重建、全部raw、CPU限制、
真实linked code、命令及失败说明见 [CO evidence](../evidence/co-count/README.md)。

## CS：CO 标量计数主线集成

在 `main` 的 README 整理提交 `820c79b` 上，仅移植已审阅 `71582f7` 的
CO 提交增量。生产 `src/`、全部 `tests/`、Cargo manifest/lock 与该固定
CO 快照逐 Git diff 相同；README 与 `820c79b` 逐字节相同。未合入 CO 的
CN 父提交，也未带入 CP SIMD、CR、PGO flags/profile 或其他实验。唯一
cherry-pick 冲突在本文件末尾，保留 CO 历史条目，移除非本次集成的 CN
上下文。原实验分支、raw、patch 和 summary 不变；上节和 CO evidence
所写“独立分支、main 未合入”描述的是当时的 CO 决策，本节更新当前状态。

本次实际运行 `python3 tools/cs_verify.py`：macOS arm64 release default
79、tool-json 130、tool-json+debug-runtime 138 tests 全部通过；前两套
未启用 debug-runtime，实际覆盖 Count 融合。原 admission/layout test
验证 descriptor 40 B、Op 48 B 及拒绝形状；`tests/co_count.rs` 对照 native
核对 stdout/status、错误时 index/fuel/buffer trace、overflow 部分进度、
effect 顺序、chunk yield、取消/弱 owner 释放、复用及 live JSON 保留。
debug 套验证原逐 Op 路径，不把它当作融合验证。stable fmt、九项 Python
workbench tests 通过，CO reducer 的 summary 与原件逐字节相同。

验证原始命令、退出码、stdout/stderr、源码 SHA-256 和实际工具链见
`evidence/cs-integration/verification.json`。所有构建使用已挂载外盘
target、原 fcntl 锁、单 job、Rust 1.96.0 offline/locked，无 PGO flags。
没有新增性能请求、Linux/Wasm 验证、依赖下载、worktree、target 或 CI。
复用 CO 的大输入合成请求信号，保留其小输入回退、CPU 采样粒度和代码/
同步栈增长限制，不推出通用提速或累加 PGO 收益。识别范围和公开接口不变。

固定签名集成提交交一次 Luna 定向只读审阅，最终 SHA、审阅与远端 main
回读记在 CS 卡片交付；CT/后续 PGO 必须引用该 SHA，不能沿用旧 profile。
