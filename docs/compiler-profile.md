# E: cold-compiler diagnosis

## Conclusion

Keep Cranelift. Between simplifying lowering and exploring a lighter backend,
prefer a bounded predicate-lowering experiment first. Backend replacement is not
justified by this data. **E implemented only diagnostics, not the candidates below.**
Single-pass allocation already has a negative old-IR/M4 result; the current
general-continuation CFG on Linux has not been retested. That history lowers its
priority rather than making it an untested first choice.

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

## Ranked candidates and historical negative evidence

Historical evidence: `/Users/yuzhe/.hermes/cache/scratch/minipy-hotspots/FINDINGS.md:3,10,14--21,33`
records four alternating ablation rounds on `564381d`'s old IR, macOS M4,
10,000 weighted rows/chunk4096, ready providers. With diagnostics omitted and
the original repeated verifier retained, backtracking versus `single_pass` gives
source-cold totals 117.604 -> 134.7705 us and prepared totals 20.2295 -> 42.792 us
(about 2.1x). These are historical source-to-result measurements, not E's Linux
compile times. They reject an unconditional allocator switch. E's larger CFG and
Linux environment are new conditions, not evidence that the regression vanished.

The order below favors plausible net absolute savings after known regression
risk. Planning ranges are hypotheses, not measured gains; allocator net benefit
is unknown and may be negative, so its larger bucket does not rank it first.

1. **Predicate-oriented conditional lowering.** Remove approximately two helper
   blocks per consumed and/or, conversions and a bool block parameter. Planning-only
   5--20 us: roughly 8% fewer weighted blocks applied to relevant traversal/regalloc
   buckets, less any fixed cost. No measured effect/bound is claimed. Minimal test:
   one local `if` lowering change, same compiler/default checks, cold-result A/B
   and bool-expression/short-circuit overflow tests. Risk: RHS skip/error order and
   join semantics. No new IR pass or metadata cache.
2. **No-suspension entry specialization.** Omit PC dispatch for statically proven
   scalar/no-yield code. Planning-only 5--15 us from a small fraction of scalar's
   75-us regalloc/41-us verifier buckets; not the 1.62-us setup. Minimal test: one
   local codegen branch, scalar cold-result A/B and print/overflow/entry tests.
   Risk: proving non-entry resume impossible under the current runtime contract.
   Reject if it requires API/threat-model changes; retain checked arithmetic and
   output errors. No second backend. Its proof burden makes it lower priority.
3. **Conditional `regalloc_algorithm=single_pass` retest, not default adoption.**
   Only if a new bounded experiment is authorized for the current general CFG on
   Linux: replace Ion/backtracking with Fastalloc; no old allocator on success,
   no extra guard/backend. The diagnostic weighted bucket is 87.57 us (75--123 us
   across nonempty cases), an addressed-cost ceiling, not expected net savings.
   The old end-to-end regression above makes both compile and execution benefit
   uncertain. Minimal validation: one setting change, matched plain-build cold
   source-to-result and prepared 10k-row execution, code bytes and existing
   semantic/lifecycle tests, with all verifier calls retained. Reject if extra
   spills/code/runtime erase any compile saving. Pinned allocator mapping is in
   `machinst/compile.rs:57--79`. No E retest or backend development was performed.

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

## F: predicate lowering experiment (2026-10-14)

**Keep the local predicate lowering.** This is the separately authorized follow-up
to E's first candidate, measured against `51e83bd2bc6f31d04ba0973b71e702f12da9134e`.
No other E candidate was implemented. `Stmt::If` sends conditions through
`Emit::predicate`: and/or create only their RHS block, `not` swaps branch targets,
and other expressions still use the existing checked value emitter. This replaces
the condition's skip/join and materialized bool parameter, rather than adding a
second pass or running both implementations. Assignment, print and other value
consumers retain `Emit::expr`. No new runtime eligibility check, fallback, cache,
allocator setting, verifier suppression, semantics, ABI or scheduler change.

### Work removed and structural evidence

Separate untimed feature-enabled compiles inventory actual pre/post CLIF. Each
cell below is baseline -> candidate; machine bytes agree with plain binaries.

| Shape | Blocks pre/post | Instructions pre/post | Params pre/post | uextend pre/post | Machine bytes |
|---|---|---|---|---|---:|
| scalar control | 10/10 -> 10/10 | 35/30 -> 35/30 | 3/3 -> 3/3 | 2/1 -> 2/1 | 260 -> 260 |
| weighted | 25/25 -> 23/23 | 100/96 -> 98/93 | 11/9 -> 12/9 | 2/2 -> 2/2 | 572 -> 548 |
| nested | 39/39 -> 31/31 | 138/126 -> 126/115 | 19/16 -> 20/17 | 8/7 -> 6/6 | 704 -> 636 |

Four resume states remain for both scan shapes, one for scalar. Removing the bool
join does not promise fewer *total* SSA parameters: forwarded loop locals add
parameters on the new edges, so nested totals increase by one. Leaf comparison
extensions remain; nested not materialization removes two pre-IR extensions.
The retained twelve CLIF dumps show the exact change. No store, budget check,
effect authorization or arithmetic failure edge was removed as an optimization.

### Measurement boundary and noise follow-up

Actual existing ARM64 Linux VM, 2 vCPU/2048 MiB, Linux 6.18.52; Rust 1.96.0,
Zig 0.15.2, musl target, pinned Cranelift 0.128.3, speed/backtracking/default
verifier/System allocator, default chunk 256. VM initially off, started with its
existing launcher in a PTY. A non-PTY launch failed before boot; no system change
was needed. Initial guest available memory ~1862 MiB, zero swap use, 12.3 GiB
free; no competing guest experiment. It was powered off after tests, vfkit exit 0,
and released to the separate G lane. G did not share this target or run in the VM.

The small example driver measures fresh owning source copy -> parse/type/lower/
codegen/install -> provider construction/row generation/import -> native execution,
both real asynchronous scheduling boundaries -> task/provider/row/code cleanup ->
returned status/stdout. Output remains live for checking outside timing. The
current-thread Tokio runtime and input source are prepared once per process.
No source/code/result cache. Prepared mode intentionally reuses one CodeUnit and
times execution, producer/import and per-request cleanup; no observed reuse rate
is assumed. CPU uses process `clock_gettime`; wall uses Instant, with JSON and
result oracle outside both. Compile attribution is wall time, not separately
measured compile CPU, and independent medians need not sum. No retained-memory
or RSS improvement is inferred from machine bytes. Plain binaries have no
`minipy_native::compiler_profile::` symbols; inventories use separate binaries.

Initial three alternating rounds: scalar/0 rows, weighted and nested/100 and
10k rows, cold and prepared, 101 requests/process. All 6060 results pass a Rust
reference stdout oracle; 12 inventories are separate. Samples 0 and 1--5 remain
first/transition records, 6--100 form the initial summary. Unchanged scalar cold
round deltas -3.0/+22.2/-23.6% show that these short processes are too noisy for
causal timing claims. Initial pooled numbers are preserved, not promoted as gains.

Exactly one follow-up used **the identical hashed binaries**, scalar/0 and both
scan shapes/10k, three alternating rounds, 1001 requests/process. It retained all
36036 observations and preselected samples 501--1000 as the longer steady-process
window (1500 observations/variant/shape/mode). `taskset` was absent, so no affinity
was set or claimed and no guest package was installed. This changes warm-up length,
not the fresh compilation on every cold request. No further timing iteration.

Follow-up pooled medians in us (observations, not confidence intervals):

| Shape | Cold wall | Cold CPU | Compile wall | Prepared wall | Prepared CPU |
|---|---:|---:|---:|---:|---:|
| scalar | 130.797 -> 130.881 | 131.000 -> 131.083 | 128.471 -> 128.445 | 1.126 -> 1.002 | 1.625 -> 1.500 |
| weighted 10k | 324.437 -> 287.923 | 324.334 -> 287.875 | 254.249 -> 219.138 | 81.073 -> 66.655 | 81.333 -> 67.000 |
| nested 10k | 466.784 -> 414.577 | 466.479 -> 414.354 | 389.355 -> 344.024 | 87.640 -> 70.907 | 87.875 -> 71.188 |

Cold wall round changes: scalar +0.16/+0.38/-0.65%, weighted -6.35/-13.29/-12.10%,
nested -10.45/-13.42/-10.15%; CPU changes track wall. This repeatable cold saving
exceeds the now-stable cold control. Weighted prepared round changes are
-19.09/-18.48/-4.19%; nested +6.74/-20.68/-22.36%. The first nested prepared
regression remains visible. Prepared scalar varies +18/-30/+9% at ~1 us despite
identical IR, so do not claim precise execution speedups or a universal runtime
improvement. The observed prepared trade-off does not erase cold savings. Keep
the small lowering replacement on these bounded source-to-result results, not
because E predicted 5--20 us or because IR/code bytes alone are smaller. This is
not a new Monty comparison, general workload result or backend-selection decision.

### Correctness, identity and reproduction

Before changing codegen, the three new regressions passed against old lowering.
They cover all eight inputs to five nested bool predicates, assignment/print
versus condition consumers, if/else joins, skipped/evaluated overflow on both
operand sides, preserved earlier stdout/no later effect, loop-carried state over
chunk=1, and ordered Tool calls after a predicate join. Candidate default and
compiler-profile macOS suites each pass 48 tests, both all-target clippy runs
pass, and the actual Linux release suite passes all 48 tests. Existing lifetime,
cancellation, late completion, permissions, cumulative budgets and error tests
are retained. One initial clippy type-complexity finding in the test table was
fixed with a test-local type alias, without a lint exemption.

`bench/results/f-predicate/` contains original/follow-up gzip raw, separately
recomputed summaries, exact source/binary hashes, commands, candidate.patch,
CLIF dumps and Linux test log. The original identity hashes the initial scripts;
the final evidence identity records the later follow-up-aware summarizer and
runner's explicit historical-baseline revision separately.
The benchmark driver was unchanged between A/B builds and the follow-up. On a
disposable checkout of F, produce baseline by reverse-applying candidate.patch
to codegen, build/copy both feature variants, then reapply it for the candidate:

```sh
git apply -R bench/results/f-predicate/candidate.patch
cargo +1.96.0 zigbuild --release --locked --target aarch64-unknown-linux-musl --example predicate_ab
cp target/aarch64-unknown-linux-musl/release/examples/predicate_ab target/f-baseline-linux
cargo +1.96.0 zigbuild --release --locked --target aarch64-unknown-linux-musl --example predicate_ab --features compiler-profile
cp target/aarch64-unknown-linux-musl/release/examples/predicate_ab target/f-baseline-ir-linux
git apply bench/results/f-predicate/candidate.patch
# Repeat the two builds/copies as f-candidate-linux and f-candidate-ir-linux.
# Runner preserves existing raw; move only the reproduction checkout's result dir aside first.
python3 tools/predicate_ab.py HOST KEY KNOWN_HOSTS HOST_KEY_ALIAS
python3 tools/predicate_followup.py HOST KEY KNOWN_HOSTS HOST_KEY_ALIAS
python3 tools/summarize_predicate_ab.py
python3 tools/summarize_predicate_ab.py followup
cargo test --locked
cargo test --locked --features compiler-profile
cargo clippy --locked --all-targets -- -D warnings
cargo clippy --locked --all-targets --features compiler-profile -- -D warnings
cargo fmt --all -- --check
git diff --check
```

Use a freshly built baseline matching the recorded codegen hash. The runner pins
the historical baseline and separately records the reproduction checkout HEAD.
Stop F here and hand off for one focused same-card review. No second optimization,
extra CI/PR, source cache, backend or independent experiment matrix was added.
