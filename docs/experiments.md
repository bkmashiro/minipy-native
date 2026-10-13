# Experiments

Status: real source-to-native execution and synthetic measurements recorded. Implementation is validated; one focused independent review is pending. These results do not establish production workload or sandbox-equivalence claims.

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
- `monty_helper`: the same QuoteBuffer data layer, an opaque handle, and a finite checked weighted/monotonic aggregation host capability, then record. Three direct host boundaries, no per-row callback and no arbitrary execution plan. The helper mode is chosen by the benchmark's fixed capability configuration. It intentionally moves the supported aggregation into trusted host code; it is a stronger simple alternative, not arbitrary user-source compilation.
- `host`: checked handwritten aggregation over the same imported buffer, including producer/import/cleanup. This is a performance floor with a different execution/authority contract, not an equivalent sandbox.

`total_ns` is measured directly around each full invocation, including producer construction and cleanup. Compile, conversion and execution columns are attribution only; their medians need not sum to the median total. Warm Monty includes its required runner clone. For native, conversion includes producer/import/provider creation. For `monty_helper`, producer/import is inside execution, so `conversion_ns=0` does not mean it is free. `code_bytes=0` for non-native rows is an unmeasured/not-applicable sentinel, not zero bytecode storage. Printing, the correctness oracle and separate retention tracing are outside single-invocation timing. Fixed input settings are batch=7 and limit=700. Delayed operation correctness is tested separately; benchmark ready completions do not represent production I/O distributions.

Four raw CSVs contain 5,976 unique samples, checked and reduced to 336 groups by `tools/summarize.py`. Per platform/opt run: 1,320 successful standalone invocations, 120 successful light tasks, six successful heavy tasks and 48 expected cancellations. Raw CSVs and derived `summary.csv` are committed; no manually transcribed sample aggregation is used.

### Linux results

Source-to-result median microseconds, 10,000 rows, `speed` optimization, native chunk=4,096:

| Source | Native cold | Native warm | Monty cold | Monty warm | Monty+helper cold | Monty+helper warm | Host |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Weighted scan | 288.793 | 78.751 | 4749.298 | 4750.652 | 85.959 | 79.230 | 70.146 |
| Stateful branch | 363.043 | 81.397 | 4764.548 | 4776.860 | 83.292 | 81.334 | 71.543 |

The fresh native compile accounts for about 219/292 µs respectively. Native producer/import is about 45 µs on this cold path; execution plus cleanup is about 24/26 µs. Ordinary Monty spends about 2.5 ms constructing/importing objects and about 2.2 ms executing/cleaning up. Avoiding that object layer is the major win. The fixed typed helper avoids it too and is approximately 3.4–4.4× faster than cold native here. Warm native is approximately tied with Monty+helper and remains slower than the direct host helper. This milestone does **not** demonstrate a source-once win over the strong helper alternative.

Empty and 100-row fresh-source execution is compile-dominated. At chunk=256 the weighted cold medians are 366/242 µs, and stateful medians are 335/306 µs. Variation across ordered cells includes process warm-up, VM scheduling and CPU frequency; these are small synthetic cells, not confidence intervals or production percentiles.

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

Keep the small typed native compiler as a research slice. The data/ownership boundary and chunked loop work, and generated code changes with source. The two finite host helpers cover these examples more cheaply for source-once execution. Do not expand language scope, implement a cache, infer representative reuse or claim an overall runtime replacement from this campaign. Compiler/lowering, dispatcher and trusted providers remain the TCB; test coverage is not a proof of safe arbitrary untrusted native execution. Exact integrated-commit review is the remaining gate, with findings to be returned separately rather than another open-ended optimization loop.
