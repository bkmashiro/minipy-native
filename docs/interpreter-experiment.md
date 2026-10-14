# G: bounded typed-IR interpreter experiment

## Decision and boundary

Keep an explicit experimental backend for low-preparation-cost scripts. Do not
replace the native default. Against fixed `51e83bd`, small source-once scripts
save substantial compilation cost and suspended tasks retain less state. At
10,000 weighted rows, cold wall time is neutral and prepared native is about
3.8x faster. The current F native improvement is outside this same-round
comparison and will change that crossover. No automatic tiering/JIT, cache,
new language features, host business kernel, or generalized opcode framework
was implemented. Explicit selection is this experiment's boundary, not a
conclusion against future hot-loop compilation or mixed-task strategies.

`InterpreterUnit::prepare_script(source, inputs)` runs the existing bounded
frontend once, binds static scalar and buffer slots and flattens statement
control flow. Expressions retain existing typed `Expr` operators. Branches and
loop PCs are private and sealed by preparation. A backward buffer-liveness
fixed point includes loop backedges. Only asynchronous/chunk boundaries need
owner reclamation. `execute_script` consumes the same inputs, `ScriptLimits`,
`Provider`, cancellation token and trace and returns the same `ScriptOutcome`.
The unit can be shared explicitly via `Rc`; locals and owners belong to a task.

Removed work: CLIF construction, SSA/backend passes, register allocation,
executable memory allocation/finalization and native code reclamation on the
interpreter path. `InterpreterUnit` does not construct `CodeUnit` or call
Cranelift. Added work: one bounded flatten/liveness preparation and statement
match/expression recursion during execution. Short-circuit and checked integer
operations preserve left-to-right failure order. A row is copied as two i64
fields into bounded state, not boxed or converted into a dynamic object.
There is no per-instruction allocation, name lookup, coroutine scheduling or
Tool descriptor construction. Chunk pre-reservation matches native fuel/error
order. No baseline execution or shadow oracle occurs in the production API.

The library runs inside a resident current-thread service/runtime, with no
per-task process, OS thread or interpreter initialization. The benchmark creates
one Tokio runtime before its request loop. Empty buffer-owner Vecs have capacity
zero and allocate nothing; an empty output String allocates only at first print.
The fixed 32-i64 scalar array avoids another task allocation and uses the existing
frontend local bound. The interpreter uses safe indexed Quote access under a
trusted frontend invariant. The shared runtime owns detached provider futures
and cancellation reception. Output formatting, permissions and Tool-call budget
checks are shared with native, not duplicated backend policies. The crate still
links/compiles Cranelift for the native API; this is not a small interpreter-only
binary packaging claim.

## Same-round Linux costs

Actual ARM64 Alpine VM, 2 vCPU/2048 MiB, existing launcher, no Docker, affinity,
package installation or system tuning. F had released the VM and restored it off.
The speed binary uses System allocation with no trace/allocation instrumentation.
Pinned ordinary Monty runs all user scalar/filter/product/sum/branch logic through
its own compiler/evaluator, with tuples/lists for Quote fields and real asynchronous
Tools. The producer constructs a distinct immutable buffer per request, within
execution timing, for every engine. Its Monty representation conversion is also
timed. No handwritten business primitive is used.

Three alternating engine-order rounds, five shapes, cold/warm, 201 requests per
process, all 18,090 raw samples retained. The last 100 per process were chosen
before execution for the summary (300 per cell). Cold includes fresh parsing,
preparation, producer/import, execution and owned executable reclamation. Warm
is explicit reuse of the same prepared code, not a cache hit assumption. Monty
clone/start is included in warm execution. Native/interpreter Rc clone is included.
Scalar expected-output construction, runtime startup, input schema, source String
ownership, JSON emission and result assertion are outside each request timer.
The source is reparsed in every cold request; there is no source cache. Code
reclamation is inside execute time, so the raw `drop_ns=0` is an attribution
placeholder, not a measured free-cost claim. An untimed one-time interpreter
preparation obtains logical prepared bytes; process-first samples are retained
but excluded from steady resident-service summaries.

Pooled median wall microseconds (round-level values and CPU are in summary.json):

| Shape | Native cold | Interpreter cold | Monty cold | Native warm | Interpreter warm | Monty warm |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| scalar | 167.123 | 27.026 | 38.659 | 4.720 | 3.425 | 22.765 |
| weighted 100 | 262.770 | 50.673 | 133.056 | 5.346 | 11.636 | 103.193 |
| stateful 100 | 284.386 | 57.228 | 108.352 | 5.890 | 13.114 | 104.648 |
| weighted 10k | 328.899 | 328.696 | 3940.481 | 79.716 | 300.340 | 3758.763 |
| stateful 10k | 357.461 | 313.077 | 3662.916 | 89.082 | 269.946 | 3612.702 |

Weighted 10k native prepare/execute medians are 259.059/69.126 us;
interpreter 28.905/296.677 us. Preparation savings are spent on interpretation,
so weighted cold is neutral, not a win. Stateful cold improvements are consistent
across these three rounds, but do not establish a universal 10k-row rule. Small
warm operations show large round noise (e.g. interpreter stateful 100 warm
20.158/7.457/13.490 us). Do not use those cells to justify low-level dispatch tuning.
This experiment stops here without a second interpreter representation or fusion
candidate. Phase attribution, not a sampled profile, identifies large-scan
execution as the next possible target; no claim localizes that time to one opcode.

## Suspended-task density and strong Monty control

Memory runs are separate allocation-diagnostic binaries and fresh processes,
with concrete per-engine task futures so an inactive Monty future cannot enlarge
an interpreter/native task. Each task independently prepares its code. All load
and record operations reach real gates before sampling. This is unique-code task
density, not explicitly shared-unit density. Input payload is 16 bytes/row/task.
The logical live-requested increment includes code, task/provider state, handles
and amortized process/compiler caches; native executable mmap is not counted by
the Rust allocator. RSS/PSS include it but also demand-paged fixed code, arena
capacity and allocator retention. Logical per-task figures are not RSS per task.

| Hold | Native requested B/task | Interpreter B/task | Ordinary Monty B/task |
| --- | ---: | ---: | ---: |
| 32 tasks, 100 rows, load wait (excluding 1600 B payload) | 8908.5 | 3220.1 | 50792.8 |
| 32 tasks, 100 rows, record wait | 8876.6 | 3188.1 | 52408.9 |
| 8 tasks, 10k rows, load wait (excluding 160000 B payload) | 11520.0 | 3322.3 | 50933.3 |
| 8 tasks, 10k rows, record wait | 11512.3 | 3314.5 | 1249789.5 |

All assertions complete and all task-owned Quote buffers disappear before record
when no future scan needs them; weak-observer correctness tests prove this for
the interpreter. Later scans keep live owners, and aggregate held buffer bytes
are charged on load. Logical interpreter prepared weighted unit bytes are 1322
(excluding allocator rounding/Rc header); actual wait increments above include
other costs. At 8x10k record wait, process RSS increments are native 3,338,240 B,
interpreter 1,830,912 B, Monty 16,203,776 B. These are one bounded gate observation
per shape, not repeated confidence estimates or maximum-density claims.

Ordinary Monty originally kept a dead list binding. A secondary strong baseline
explicitly inserts `quotes=[]` immediately before the terminal record in these
fixed benchmark shapes only. It does not alter arithmetic or Tool effects. This
is benchmark-only last-use control, not a runtime source rewriter. It has its own
harness/binary identity, alternating plain/clear same-round comparison, 4824 raw
speed samples and 16 memory rows. Weighted 10k cold plain/clear medians are
3907.565/3936.832 us; warm 3793.534/3773.822 us (neutral). At 8x10k record wait,
clear reduces live-requested increment per task from 1,249,789.5 to 1,220,837.5 B,
not to interpreter state size. At 32x100 it goes from 52,408.9 to 51,808.9 B.
Arena/capacity retention remains part of the physical/requested-live cost even
when a language-level list is released. Do not label every retained allocator
byte as a logically live Quote object or a leak. Reclaimed phase and physical
retention are recorded; no workaround was added to Monty to manufacture a loss.

## Correctness, identity and reproduction

Existing 45 core tests plus initial seven interpreter suites passed during the
macOS and actual Linux ARM64 release campaign (52 tests). Two later suites add
provider load/record failure and dropped-task-receiver checks. Review of `ac08ca1`
then reproduced a flaky chunk-cancellation wait, invalidating its reproducible
54-test macOS acceptance claim. The test-only repair below passes all 54 macOS
release tests (interpreter 9). Linux's 52-test result remains historical: neither
the repaired tests nor the two added suites have been rerun on Linux.
The production runtime/interpreter did not change after Linux qualification.
Clippy all core targets with `-D warnings`, fmt and diff checks pass. Two common
Monty benchmark tests pass. Tests compare status/stdout, Tool counts and trace
phase/buffer/fuel/index against native, including sequential/conditional Tools,
zero/short scans, persistent fuel, live future scans, overflow and short-circuit,
atomic stdout bounds, real load/record suspension, token cancellation, chunk
cancellation, late completion and last-use owner release. Ordinary user errors
become TaskError, not worker panics. Private PC/row invariant failures remain TCB
programmer errors, not an untrusted-execution claim.

### Review repair: deterministic chunk cancellation

Before the repair, the focused release command failed at `tests/interpreter.rs:281`
with no scan yield observed. `TestProvider` awaits `sleep(Duration::ZERO)` even
with default delays; 1,000 `yield_now` polls could finish before its timer advanced.
The test now uses the existing gated provider with configurable rows. It observes
load entry, releases the gate, and directly polls the real execution future until
the first actual chunk suspension, then stops polling before issuing cancellation.
The gate/counter helper's other fixed-spin waits now have a five-second timeout,
as does the chunk handshake. This is a test failure bound, not an inserted delay.
No production synchronization, trace overhead or scheduler change was added.

Assertions prove exactly one yield at index 1, one row of cumulative fuel charged,
16,000 buffer bytes still owned before cancellation, `Cancelled` status, empty
stdout, no record dispatch and owner release afterward. Reuse of the same prepared
unit still produces independent results for one-row and three-row requests.

On macOS 26.4.1 / Rust 1.93.1, the focused release test passed once, then 50 consecutive
fresh invocations passed. All nine interpreter suites passed in 10 consecutive
invocations. `cargo test --locked --release` passed all 54 tests;
`cargo clippy --locked --all-targets -- -D warnings`, `cargo fmt --check` and
`git diff --check` passed. Commands and updated test hash are recorded in
`bench/results/g-interpreter/final-validation.json`. The performance campaign,
raw evidence and production sources are unchanged; no VM, main or CI changes.

Evidence: `bench/results/g-interpreter/` contains compressed raw, identity,
Linux test log and recomputable summary. `primary-harness.rs.txt` has exactly the
SHA-256 of the original primary benchmark harness; the final harness additionally
supports monty-clear. Core/source/dependency hashes identify the measured snapshot;
final test-only edits are explicitly later than the primary identity. Do not
combine results from F or previous Monty campaigns with this fixed baseline.

Commands from this worktree:

```sh
cargo test --locked --release
cargo clippy --all-targets -- -D warnings
CARGO_TARGET_DIR="$PWD/target" cargo +1.96.0 build --manifest-path bench/Cargo.toml --bin interpreter_ab --release --locked
CARGO_TARGET_DIR="$PWD/target" cargo +1.96.0 zigbuild --manifest-path bench/Cargo.toml --bin interpreter_ab --release --locked --target aarch64-unknown-linux-musl
# Copy the plain executable as target/g-speed-linux; similarly copy the
# --features allocation-diagnostics build as target/g-memory-linux.
cargo zigbuild --tests --release --locked --target aarch64-unknown-linux-musl --message-format=json > target/g-test-build.jsonl
python3 tools/interpreter_ab.py HOST SSH_KEY KNOWN_HOSTS
# For the original primary matrix use the exact preserved primary harness.
# Final harness copies: target/g-clear-{speed,memory}-linux.
python3 tools/interpreter_clear_ab.py HOST SSH_KEY KNOWN_HOSTS
python3 tools/summarize_interpreter_ab.py > bench/results/g-interpreter/summary.json
```

The campaign refuses to overwrite existing raw. All remote executable hashes
were read back before execution. VM shutdown returned vfkit exit 0 and
`VM is stopped`, restoring its initial off state. No changes to Pysolate source,
main, CI, global configuration or gateway are part of G. Publication is only the
independent experimental branch. Host/gated serialized costs are evidence for
this bounded slice; sustained arrivals, mixed-task tail latency, shared-unit
fleet density, compiler contention and hot-loop switching were not measured.
Those require a separately authorized workload and state-transfer design.
