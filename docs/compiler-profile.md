# E: cold-compiler diagnosis

## Conclusion

Keep Cranelift. Between simplifying lowering and exploring a lighter backend,
prefer a bounded local experiment first. An even cheaper first probe is Cranelift's
existing single-pass allocator. Backend replacement is not justified by this data.
**Only diagnostics were implemented; all candidates below remain untested.**

Ordinary-release empty/weighted/two-scan compiles cost 121.63/330.12/459.25 us.
ISA/module setup and finalize/load cost only a few us. Backend work has both a
substantial per-function floor and an IR-dependent component. Empty still includes
our frame/PC dispatch, so it does not establish a bare-Cranelift fixed minimum.
Six shapes cannot identify a universal intercept or causal cost/instruction;
scan-only is slower than weighted in this noisy run. Do not fit a monotonic model.

## Measurement contract

- Baseline `4abb2a10523b4c3b72a6f41b3498d599394d0cc5`; exact measured source/binary
  hashes and all commands in `bench/results/e-compiler-profile/identity.json`.
- Actual existing Linux ARM64 VM on M4: Linux 6.18.52, 2 vCPU/2048 MiB. Initially
  off, started with the existing launcher, restored off after work. Preflight:
  ~1864 MiB available, zero swap use, 12.4 GiB disk free, no competing experiment.
- Rust 1.96.0, Zig 0.15.2, `aarch64-unknown-linux-musl`, Cranelift 0.128.3,
  `opt_level=speed`, backtracking regalloc, default verifier enabled, System allocator.
- Timer: source already in memory -> owning finalized `CodeUnit::compile_script`
  result. Every sample reparses/types/lowers/compiles. No source/code/result cache.
  `drop_ns` separately times returned CodeUnit/JIT destruction. Compile+drop is the
  per-sample sum of those intervals. Source I/O, JSON and execution are excluded.
- Three serial rounds, six shapes, plain/diagnostic builds interleaved by case;
  round 1 reverses build and case order. Each of 36 fresh processes records first
  compile (sample 0), then 60 new compiles. Samples 1--5 are retained as transition;
  tables use 6--60: 165 observations per case/build. First compile follows argv/
  source/profiler setup, not OS boot. Tokio starts only in untimed validation.
- Each process then executes actual generated code with scheduling-boundary Tool
  providers and a Rust reference result/stdout oracle: 100 rows, batch 7, limit 700.
  The new two-scan fixture takes its positive branch; existing tests cover both
  conditional Tool branches and chunk/error/budget paths. M4 scalar/two-scan quick
  probes were separate and are not pooled into Linux numbers.

## Compiler phases and instrumentation cost

`compiler-profile` enables the pinned public `timing::Profiler`/
`set_thread_profiler` API via `compile-timing`. A diagnostic-only RAII recorder
provides nanosecond inclusive/exclusive durations; upstream formatted tables round
to milliseconds. Token allocations, clocks and TLS/RefCell updates add real cost.
Inventories/IR formatting occur in separate untimed compiles. Without the feature,
all new timers/TLS/inventory code is compiled out: the plain binary has zero
`minipy_native::compiler_profile::` symbols, and every timed compile asserts empty
`CodeUnit.clif`. No safety check or optimization setting was changed.

All following times are medians in us. Frontend includes admission, parse and type;
parenthesized parse/type are its children. Setup includes ISA/JIT/signature;
lowering includes liveness/SSA/sealing; define includes backend plus JIT allocation/
copy/install; finalize includes relocation/protection/get-finalized-function.
Scratch clearing/return costs another 0.96--2.38 us; unassigned gaps 2.33--4.42 us.

| Case | Plain compile | Diagnostic compile | Diagnostic frontend (parse/type) | Setup | Our lowering | Define | Finalize/load | Plain drop |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| empty | 121.63 | 157.72 | 3.54 (1.25/0.67) | 3.79 | 2.92 | 134.80 | 4.04 | 5.42 |
| scalar | 227.46 | 251.72 | 24.51 (19.96/2.88) | 1.62 | 5.08 | 210.91 | 4.04 | 3.75 |
| scan | 397.22 | 396.73 | 44.06 (33.34/7.96) | 1.21 | 8.13 | 335.75 | 3.00 | 4.42 |
| weighted | 330.12 | 354.88 | 41.14 (34.85/4.71) | 1.12 | 9.25 | 296.06 | 2.67 | 2.58 |
| two_scan | 459.25 | 470.79 | 55.44 (46.68/7.09) | 1.04 | 12.67 | 396.06 | 2.42 | 3.38 |
| legacy | 306.19 | 353.50 | 47.10 (41.81/3.63) | 1.12 | 8.46 | 287.73 | 2.54 | 2.38 |

Plain first weighted compiles are 822.30/738.81/819.04 us across processes;
scalar 657.49/531.36/879.94 us. Transition samples 1--5 have medians 451.53 and
289.94 us respectively. These are distinct from scratch-warm **fresh-source**
compiles in the table. All cases' first/transition/compile+drop data are in summary.

Round-matched diagnostic/plain differences:

| Case | Round 0 | Round 1 | Round 2 |
|---|---:|---:|---:|
| empty | -0.7% | +29.1% | +39.5% |
| scalar | +16.6% | +2.1% | -0.9% |
| scan | +1.6% | +20.9% | -4.5% |
| weighted | +8.2% | +8.1% | +4.9% |
| two_scan | -6.4% | +10.5% | +4.1% |
| legacy | +18.7% | +15.1% | +16.6% |

Instrumentation and process variation are material. Negative differences do not
prove profiling is free. Apply no universal correction. Pass costs below belong
to diagnostic builds, not plain-release times; these are observations, not service
capacity or confidence intervals. Independent medians need not sum to a total.

## Three identified backend hotspots

Define occupies median 82.1--85.8% of whole compile wall. Exclusive pass medians
and median per-sample shares (backend / whole compile):

| Pass | Weighted us | Weighted shares | Two-scan us | Two-scan shares |
|---|---:|---|---:|---|
| regalloc | 87.57 | 29.7% / 24.6% | 122.96 | 31.1% / 25.9% |
| verifier | 75.74 | 25.7% / 21.3% | 107.17 | 27.1% / 22.7% |
| egraph | 28.80 | 9.7% / 8.1% | 40.68 | 10.0% / 8.4% |

Root compile self is another 36.18/38.81 us, but includes untimed internal work and
profiler bookkeeping, not an identified fixed setup. Flowgraph is 23.34/26.47 us,
VCode lowering 13.92/23.09, emission 9.92/14.55. Full pass records are in raw/summary.

Root compile **inclusive** contains the children. Never add it to their inclusive
times. The summarizer checks every diagnostic sample: sum(exclusive passes) equals
root inclusive; define wall >= root inclusive; compile wall >= non-nested phase
sum. Flowgraph/domtree calls nested in verification are already subtracted from
verifier self time. Parse/type likewise must not be added again to frontend.

There are exactly five verifier calls per compile: `compile_stencil` entry,
after legalization, after unreachable elimination, after constant-phi removal,
and inside `egraph_pass`. `Verifier::new` builds expected CFG/domtree.
No safe redundancy has been proved: all five remain enabled, and disabling them
is not a candidate. Upstream references: 0.128.3 `src/context.rs:138`, `:278`,
`:306`, `:337`, `:391`,
`src/verifier/mod.rs:277--319`, `src/machinst/compile.rs:57--79`.

## IR inventory and lowering audit

Pre: after FunctionBuilder finalize; post: after backend optimization, still CLIF,
not VCode. Instructions exclude value aliases. Params sum block parameters; states
include entry. Slots are declared scalar/buffer slots, not backend spills/stack
bytes. Tools use frame/status suspension, so calls below count print ABI calls.
Dead CLIF loads can be removed at machine lowering; counts do not measure executed
memory traffic. Machine bytes match plain/diagnostic in every round.

| Case | Blocks pre/post | Inst pre/post | Params pre/post | Loads pre/post | Stores pre/post | Calls | Slots scalar/buffer | States | Code bytes |
|---|---|---|---|---|---|---:|---|---:|---:|
| empty | 6/4 | 17/11 | 3/3 | 3/3 | 2/2 | 0 | 2/0 | 1 | 44 |
| scalar | 10/10 | 35/30 | 3/3 | 4/4 | 2/2 | 2 | 3/0 | 1 | 260 |
| scan | 24/24 | 89/85 | 11/9 | 14/13 | 12/12 | 1 | 4/1 | 3 | 568 |
| weighted | 25/25 | 100/96 | 11/9 | 15/14 | 17/17 | 1 | 5/1 | 4 | 572 |
| two_scan | 41/41 | 168/159 | 15/13 | 22/20 | 31/31 | 2 | 5/1 | 6 | 932 |
| legacy | 24/23 | 95/90 | 11/9 | 14/13 | 17/17 | 0 | 4/1 | 4 | 448 |

- `src/codegen.rs:lower_into` (`:180`, `:191`, `:218`): loads every scalar slot,
  creates shared error exits, always builds PC dispatch. Scalar still has ten
  post-optimization blocks despite no suspension. Some entry loads are dead.
- `Flow::expr` (`:586`): value-producing short-circuit rhs/skip/join, conversions
  and a bool parameter precede an if branch. Predicate-only consumers may omit
  that materialization without changing RHS skip/error order.
- `Flow::scan`, `live_loop`, `spill`, `suspend` (`:494`, `:336`, `:396`, `:406`):
  checked arithmetic, row budgets, chunk/yield backedges and frame/status/view
  accesses implement real semantics. Weighted chunk writeback already spills only
  total; Tool stores encode actual arguments/continuations. Do not label general
  state or all stores waste. Backend removes forwarded SSA parameters, but leaves
  most scan blocks. Twelve exact pre/post dumps are committed in the evidence dir.

## Ranked untested candidates

1. **Existing `regalloc_algorithm=single_pass` A/B.** Replace Ion/backtracking with
   Fastalloc; no old allocator on success, no extra guard/backend. Addressed
   weighted bucket: 87.57 us (75--123 us across nonempty cases), a zero-cost ideal
   ceiling rather than predicted saving. A planning target is tens of us, but this
   profile cannot forecast the difference. Minimal validation: one setting change,
   matched plain-build cold source-to-result and prepared 10k-row execution, code
   bytes and existing semantic/lifecycle tests; keep verifier enabled. Risk: worse
   spills/code/runtime can erase compile savings. Pinned mapping is in
   `machinst/compile.rs:57--79`. One bounded session, not backend development.
2. **Predicate-oriented conditional lowering.** Remove approximately two helper
   blocks per consumed and/or, conversions and a bool block parameter. Planning-only
   5--20 us: roughly 8% fewer weighted blocks applied to relevant traversal/regalloc
   buckets, less any fixed cost. No measured effect/bound is claimed. Minimal test:
   one local `if` lowering change, same compiler/default checks, cold-result A/B
   and bool-expression/short-circuit overflow tests. Risk: RHS skip/error order and
   join semantics. No new IR pass or metadata cache.
3. **No-suspension entry specialization.** Omit PC dispatch for statically proven
   scalar/no-yield code. Planning-only 5--15 us from a small fraction of scalar's
   75-us regalloc/41-us verifier buckets; not the 1.62-us setup. Minimal test: one
   local codegen branch, scalar cold-result A/B and print/overflow/entry tests.
   Risk: proving non-entry resume impossible under the current runtime contract.
   Reject if it requires API/threat-model changes; retain checked arithmetic and
   output errors. No second backend. Its proof burden makes it lower priority.

## Reproduce and verify

```sh
cargo +1.96.0 zigbuild --release --locked --target aarch64-unknown-linux-musl --example compiler_profile --features compiler-profile
cp target/aarch64-unknown-linux-musl/release/examples/compiler_profile target/e-profile-diagnostic-linux
cargo +1.96.0 zigbuild --release --locked --target aarch64-unknown-linux-musl --example compiler_profile
cp target/aarch64-unknown-linux-musl/release/examples/compiler_profile target/e-profile-plain-linux
python3 tools/profile_compiler.py root@192.168.64.3 /Users/yuzhe/.hermes/tmp/pysolate-durable/vm/command-key /Users/yuzhe/.hermes/tmp/pysolate-durable/vm/known_hosts 192.168.64.2
python3 tools/summarize_compiler_profile.py > bench/results/e-compiler-profile/summary.json
cargo test --locked
cargo test --locked --features compiler-profile
cargo clippy --locked --all-targets -- -D warnings
cargo clippy --locked --all-targets --features compiler-profile -- -D warnings
cargo fmt --all -- --check
git diff --check
```

Runner refuses to overwrite existing raw: use a fresh checkout/result directory.
Default and feature-enabled macOS suites each pass 45 core tests, both clippy
variants pass; 36 Linux processes plus six inventory oracles execute actual code.
The stdlib summarizer checks all 2208 rows (2196 compiles + 12 inventories), timer
nesting and code sizes. Compressed raw is ~133 KiB; complete initial evidence ~296
KiB. No new Linux full-suite, C/D/Monty campaign, sampling setup, CI, backend,
optimization, cache, verifier suppression or runtime/string/scheduler change.
This diagnosis does not prove an optimal system or predict other workloads.
