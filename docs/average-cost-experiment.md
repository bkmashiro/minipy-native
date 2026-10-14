# H: explicit interpreter integration and bounded average-cost experiment

## Decision

Keep the reviewed G interpreter alongside F native predicate lowering. The only merge conflict was the appended experiment log; both sections are preserved. Integration is signed merge `b3a12fdb34a319816351ca7801571d15be04be07`, parents `8cb905b` and `9b28716`. Native `src/codegen.rs` is unchanged from F, interpreter/runtime production source is unchanged from reviewed G. The CLI remains native by default. Library callers explicitly choose `CodeUnit::compile_script` + `runtime::execute_script` or `InterpreterUnit::prepare_script` + `interpreter::execute_script`. No automatic policy, cache, JIT threshold, tier transition or OSR is added.

Revert the single candidate (empty-else Jump elimination). Its small CPU differences are inconsistent across mix, provider timing and prepared execution; some small-shape cells regress. Retain the differential test and negative raw evidence. Do not add another candidate or change the language/authority contract.

## Measurement contract

- Actual existing Alpine ARM64 VM on M4, 2 vCPU / 2048 MiB. Started from off, no competing experiment; initially 1857 MiB available, zero swap use, 11.9 GiB disk free. VM returned off after final Linux tests.
- Core/test compiler Rust 1.93.1; speed/memory benchmark Rust 1.96.0, Zig 0.15.2, target `aarch64-unknown-linux-musl`, Cranelift 0.128.3 default speed/backtracking/verifiers/System allocator. Monty pinned to `56949a4c45f51cedc7a96450782f58a8017e114d`, ordinary source execution and actual Python tuples. G's `monty-clear` source releases the list after the scan, as a secondary waiting-memory baseline. No handwritten business kernel, prefiltered data or fabricated trace.
- Resident single-thread current-thread Tokio. Source text is resident; every cold request parses, validates, independently prepares, executes and drops owning code. No source cache or implicit same-code sharing. Warm is explicitly labeled: five prepared units are reused, with Monty clone included. This is not an assumed production hit rate.
- Cold includes frontend/flatten or native compilation, provider allocation/freeze/import, actual generated or interpreted computation, Monty object conversion, Tool waits, bounded output, scheduling and task/code reclamation. JSON is after each batch; process startup and corpus construction are outside. Mixed output oracle is computed before timing; output assertion remains inside, identically for all engines. Speed is uninstrumented. Allocation/physical snapshots use a separate diagnostic binary.
- Shapes: scalar, weighted 100, stateful 100, weighted 10k and cross-row stateful 100k rows. Light weights 24/8/4/2/2, scan weights 8/16/8/4/4, each 40 tasks. Scalar fractions 60% and 20% deliberately expose different break-even regions. These are controlled synthetic mixes, not an observed agent-traffic distribution.
- Planned arrivals are 500 us apart within each finite 40-arrival window, permuted by `(arrival*17)%40` to intersperse long work and short tasks. Window budget 100 ms, nominal 400 requests/s averaged across that window, with a 2000/s within-window burst. Latency starts at planned arrival, including timer granularity, compiler/scan queueing and late wakes. No task survives into another batch. If a window overruns, it is marked overloaded and the next finite batch begins only after draining: this is NOT sustained fixed-rate admission, a stable-capacity bound or an unbounded backlog test.
- Three serial rounds rotate/reverse engine ordering. Each mixed process has 21 batches, preregistered summary samples 3..20; standalone shape processes have 101 requests, summary samples 21..100. All first/transition observations remain in raw. Mixed CPU/task is total process CPU across retained batches divided by all their 40 tasks, not averaged shape medians or duplicated batch counters. Tail quantiles are nearest-rank pooled request quantiles; per-round summaries remain available.

## Integrated baseline: measured average CPU and tails

CPU and mean latency in us; short = scalar and 100-row scans. This is one same-machine campaign; no statistical-confidence or general capacity claim.

| Mix / mode / delay us | Engine | CPU/task | Mean latency | Short p95 | Short p99 | Overloaded windows / 54 |
|---|---|---:|---:|---:|---:|---:|
| light / mixed-cold / 0 | native | 698.88 | 10627.59 | 19725.12 | 23731.47 | 0 |
| light / mixed-cold / 0 | interpreter | 578.93 | 2114.47 | 1873.92 | 2508.45 | 0 |
| light / mixed-cold / 0 | monty | 1855.33 | 37722.23 | 70262.85 | 72352.12 | 0 |
| light / mixed-cold / 1000 | native | 730.55 | 13489.17 | 27433.85 | 30888.07 | 0 |
| light / mixed-cold / 1000 | interpreter | 587.95 | 3804.13 | 6979.14 | 8194.89 | 0 |
| light / mixed-cold / 1000 | monty | 1963.86 | 38761.03 | 78939.06 | 81688.57 | 0 |
| light / mixed-warm / 0 | native | 176.04 | 1201.85 | 1927.65 | 2188.93 | 0 |
| light / mixed-warm / 0 | interpreter | 537.74 | 1830.71 | 1525.29 | 2023.20 | 0 |
| light / mixed-warm / 0 | monty | 1992.51 | 39121.46 | 75756.88 | 78161.91 | 0 |
| scan / mixed-cold / 0 | native | 889.17 | 20364.02 | 29278.33 | 32476.14 | 0 |
| scan / mixed-cold / 0 | interpreter | 895.70 | 4555.03 | 2734.40 | 3618.44 | 0 |
| scan / mixed-cold / 0 | monty | 3903.45 | 100008.77 | 152033.98 | 153699.20 | 54 |
| scan / mixed-cold / 1000 | native | 885.91 | 22910.49 | 33783.07 | 36180.48 | 0 |
| scan / mixed-cold / 1000 | interpreter | 895.90 | 7986.73 | 7843.26 | 8726.77 | 0 |
| scan / mixed-cold / 1000 | monty | 3964.23 | 111015.45 | 172044.39 | 176018.19 | 54 |
| scan / mixed-warm / 0 | native | 265.81 | 1314.35 | 2081.77 | 2594.43 | 0 |
| scan / mixed-warm / 0 | interpreter | 850.71 | 3887.14 | 2034.44 | 2471.15 | 0 |
| scan / mixed-warm / 0 | monty | 4090.93 | 107635.46 | 172070.50 | 182046.29 | 54 |

Across ALL raw (including transition) windows, 199 overrun the finite 100 ms budget. Do not hide those cells by silently lowering the offered workload or dropping overrun samples. Warm excludes preparation and thus answers a different question. Ready provider operations still pass through the real detached Tokio/oneshot ownership path; delayed cells use two real 1000 us sleeps per scanning request (actual wakes are not forced to exactly 1 ms).

The light cold interpreter lowers CPU/task vs native by 17.2% ready and 19.5% delayed, and sharply reduces the measured short-task tail. In the scan-heavy cold mix the CPU totals are roughly equal (interpreter +0.7% ready, +1.1% delayed), while native has substantially longer cold short-task queueing. Prepared native saves CPU in both mixes. Monty's non-preemptible large scans make its short-task tails much larger here and overload scan-heavy windows; the native/interpreter chunk limits and true cancellation behavior remain unchanged. These observations support explicit backend choice, not a universal interpreter default or automatic policy.

## Shape-level cost ledger

Standalone source-cold arithmetic means in us. CPU is whole request; preparation and execution are measured wall subintervals, not an additive decomposition of CPU. Different process/order noise must not be substituted for the cumulative mixed CPU result.

| Shape | Engine | CPU | Preparation wall | Execution wall | Whole wall |
|---|---|---:|---:|---:|---:|
| scalar:0 | native | 216.53 | 208.55 | 5.90 | 215.77 |
| scalar:0 | interpreter | 54.55 | 43.82 | 7.14 | 52.79 |
| scalar:0 | monty | 66.00 | 38.00 | 25.38 | 65.20 |
| weighted:100 | native | 381.12 | 368.83 | 10.47 | 380.49 |
| weighted:100 | interpreter | 78.08 | 61.86 | 13.83 | 77.27 |
| weighted:100 | monty | 215.94 | 52.34 | 161.19 | 215.23 |
| stateful:100 | native | 427.66 | 413.32 | 12.68 | 427.11 |
| stateful:100 | interpreter | 90.02 | 75.52 | 12.26 | 89.20 |
| stateful:100 | monty | 202.42 | 45.58 | 154.18 | 201.76 |
| weighted:10000 | native | 387.31 | 302.76 | 83.18 | 386.88 |
| weighted:10000 | interpreter | 347.06 | 39.86 | 306.12 | 346.74 |
| weighted:10000 | monty | 3940.42 | 31.31 | 3909.21 | 3941.55 |
| stateful:100000 | native | 843.88 | 275.31 | 567.86 | 843.98 |
| stateful:100000 | interpreter | 2629.60 | 44.04 | 2585.66 | 2630.46 |
| stateful:100000 | monty | 34950.92 | 43.35 | 34920.10 | 34964.34 |

The remaining cold-small native cost is compilation; the existing E phase profile identified backend define/regalloc/verifiers, but H does not reinterpret E diagnostic pass timers as plain-release costs. Interpreter removes native compilation entirely on caller-selected success, at the expense of typed expression/statement dispatch. It is not a native compile followed by shadow interpretation. At 100k stateful rows, cold native uses 843.88 us CPU vs interpreter 2629.60 us. There is no source-independent crossover threshold: row count, body complexity, provider delay, preparation reuse and queueing all matter. A 10k weighted cold cell is much closer. Per-request cold mean preparation remains in the ledger rather than being amortized away.

## Single falsifiable candidate: empty-else Jump elimination

- Replaces: `flatten(Stmt::If)` always appends an unconditional jump after the true arm, even when the false arm is empty.
- Removed work: one retained `Op::Jump` per such if, and one VM dispatch for every successful predicate execution. The false target goes directly to the common following statement. Weighted scan has an empty else; many stateful branches have real else arms and are unaffected.
- Added work: one `no.is_empty()` preparation-time check and branch target patch. No runtime guard, fallback, second execution, allocation pipeline or additional safety check. Liveness still uses the same flattened graph; no last-use release, yield, fuel, overflow, authorization, stdout or effect check is removed.
- Correctness: candidate passed all ten macOS interpreter tests, including a new native/interpreter differential case with nested empty else, both true/false inputs, scan chunks 1/2/256 and continuation through load/record/print. Final reverted integration additionally passes all 58 tests on actual Linux and macOS. `candidate.patch` records the exact discarded implementation.

| Mix / mode / delay | Base CPU/task us | Candidate CPU/task us | Change |
|---|---:|---:|---:|
| light/mixed-cold/0 | 578.935 | 565.665 | -2.29% |
| light/mixed-cold/1000 | 587.949 | 584.819 | -0.53% |
| light/mixed-warm/0 | 537.737 | 533.152 | -0.85% |
| scan/mixed-cold/0 | 895.696 | 881.288 | -1.61% |
| scan/mixed-cold/1000 | 895.901 | 898.477 | +0.29% |
| scan/mixed-warm/0 | 850.709 | 861.553 | +1.27% |

The nominal improvements are only 0.5--2.3%, with delayed scan-heavy and prepared scan-heavy regression. Per-round signs are not uniform across all cells; unchanged scalar control and small scans also vary. Weighted-100 cold mean CPU grows 78.08 -> 99.91 us; weighted-10k cold grows 347.06 -> 368.80 us, while its prepared cell improves. Stateful-100k cold improves 2629.60 -> 2505.33 us, but that does not establish a universal benefit from the removed jump. Load-wait requested allocation saves only 128 bytes/light batch and 256 bytes/scan batch (3.2/6.4 bytes per arrived request); this does not justify a permanent runtime change without stable end-to-end value. Revert, preserve raw, stop. No more attempts are scheduled.

## Waiting memory and reclamation

Fresh diagnostic process per mix/engine. All 40 arrivals are submitted together for this separate density probe; 16 (light) or 32 (scan) tool-using tasks are held. Scalars complete. Therefore normalizing by 40 is per ARRIVED request, not per live waiting task. Each cold task owns its independently prepared code. Runtime/JIT code mappings and compiler scratch are not all counted by requested allocations, so physical snapshots are also required.

| Mix | Engine | Load live increment B | Quote payload B | Record live increment B | Reclaimed live increment B | Load RSS B | Record RSS B | Sampled high-water RSS B |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| light | native | 3721082 | 3539200 | 181436 | 28584 | 9027584 | 5476352 | 9027584 |
| light | interpreter | 3625780 | 3539200 | 86134 | 4098 | 7176192 | 3624960 | 7180288 |
| light | monty | 4521660 | 3539200 | 27374494 | 4442 | 10055680 | 38998016 | 78274560 |
| light | monty-clear | 4521884 | 3539200 | 26206590 | 4442 | 10047488 | 37249024 | 81141760 |
| scan | native | 7381104 | 7078400 | 301874 | 29964 | 12820480 | 5718016 | 12820480 |
| scan | interpreter | 7203494 | 7078400 | 124264 | 4226 | 10776576 | 3674112 | 10780672 |
| scan | monty | 8774686 | 7078400 | 54480352 | 4442 | 13983744 | 71577600 | 129470464 |
| scan | monty-clear | 8775134 | 7078400 | 52144544 | 4442 | 13967360 | 68079616 | 129847296 |

Quote bytes are 16 per row: 3,539,200/light and 7,078,400/scan at load wait, no live Quote payload after scans at record wait. The Monty-clear secondary removes its buffer list; its wait snapshot/arena still costs substantially more than typed tasks. The ordinary and clear results are both retained rather than calling the strongest baseline absent. Requested memory after reclamation includes residual runtime/compiler capacity and remaining harness handles/schedule; it is not a leak claim. RSS/PSS/high-water are observed process values, not a maximum concurrent-service task count. Code is retained through the final print, so these numbers do not extrapolate to the legacy terminal-record early-code-drop path. Native JIT mappings are outside the counting allocator; interpret both tables together.

## Strategy boundary and stop point

For cold tiny scripts, caller-selected interpreter is the strong simple alternative to native compilation. For prepared reuse and sustained long scans, caller-selected native remains valuable. The public APIs expose those choices without predicting future buffer lengths, observing hotspot counters, migrating live state, recompiling inside the scan or caching arbitrary user sources. No-suspension native specialization and native liveness/SSA rewrites would require their own isolated replacement hypothesis; H implements neither. The reverted Jump experiment already establishes a bounded stop point.

This task adds no packages, strings/bytes language path, generic optimizer, scheduler rewrite, framework, Docker, paid infrastructure, CI workflow or Pysolate change. All tool/cancellation/budget/error contracts are unchanged. No result is presented as observed production demand or Linux cross-build-only execution.

## Reproduction and verification

Raw and source/binary identity: `bench/results/h-average/{identity.json,linux.jsonl.gz,memory.jsonl,summary.json,candidate.patch}`. The base core hash is the integrated merge; candidate core is exactly that base plus candidate.patch. Both use the same measured harness. Build speed and memory separately for each base/candidate tree:

```sh
cargo +1.96.0 zigbuild --manifest-path bench/Cargo.toml --release --locked --target aarch64-unknown-linux-musl --bin interpreter_ab
# copy speed to target/h-{base,candidate}-speed-linux
cargo +1.96.0 zigbuild --manifest-path bench/Cargo.toml --release --locked --target aarch64-unknown-linux-musl --bin interpreter_ab --features allocation-diagnostics
# copy memory to target/h-{base,candidate}-memory-linux
python3 tools/average_ab.py HOST KEY KNOWN_HOSTS
python3 tools/summarize_average_ab.py
cargo zigbuild --tests --release --locked --target aarch64-unknown-linux-musl --message-format=json
cargo test --release --locked
cargo test --release --locked --features compiler-profile
cargo clippy --all-targets --locked -- -D warnings
cargo clippy --all-targets --locked --features compiler-profile -- -D warnings
cargo fmt --check
```

Runner refuses to overwrite raw; use a fresh output directory for reproduction rather than replacing historical measurements. Linux test binaries were copied, SHA256 read back and actually run with `--test-threads=1`; five suites total 58 passed. `linux-tests.txt` and `linux-test-identity.json` preserve execution evidence. Mac release default/diagnostic each pass 58; core clippy default/diagnostic, formatting and diff checks pass. Benchmark strict clippy reports G's pre-existing large `Prepared` enum and cfg-constant runtime assertion; a narrowly scoped invocation allowing those two intentional benchmark diagnostics passes. No boxing, timed-source mutation or blanket lint suppression was introduced. Do not describe that initial strict benchmark invocation as passing.

One final focused fixed-commit review is required; publication/review outcome belongs in the task handoff, without a second optimization campaign.
