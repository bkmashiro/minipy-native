# Experiments

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
