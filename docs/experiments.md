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

