# Experiments

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

### BK private complete-source obstacle inventory

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

