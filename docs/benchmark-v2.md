# Benchmark v2: cost, density and load

This lane adds a small executable suite, not runtime changes. The main comparison
is native compilation versus ordinary Monty executing the same user statements.
The historical `quote-bench generic` arm remains a finite hostcall reference.
It does not decide whether general source compilation is worthwhile.

## Contract and commands

Question: where do cold compilation, prepared execution, scheduling and retained
payload dominate? The two workloads are `weighted_scan.py` and
`stateful_branch.py`. Both are parsed/compiled; no handwritten scan is executed
in a benchmark arm. The Monty adapter removes native annotations, maps Quote
fields to tuple indices, preserves the async function and awaits, and calls main.
The input integers are within both implementations' supported semantics.

Build and local smoke:

```
cargo +1.96.0 build --release --locked --manifest-path bench/Cargo.toml --bin bench-v2 --bin memory-v2
python3 tools/run_bench_v2.py --profile smoke --skip-build --out bench/results/v2-smoke-macos-final
python3 -m unittest discover -s tools -p test_bench_v2.py
cargo +1.96.0 test --locked --manifest-path bench/Cargo.toml
```

Use a fresh output directory for each run. `run_bench_v2.py` serializes commands;
no agents or VM provisioners are started. `--profile campaign --rounds 3` is a
bounded starting campaign (39 commands per round), not an exhaustive matrix.
`--skip-build --bin-dir DIR` works on the measured Linux host with deployed
binaries and `MINIPY_FIXTURES` set to its fixture directory. Cross-builds are not
runs. Inspect host load and competing builds before measurement.

For the downstream runtime comparison, build this identical harness against
baseline `564381d` and the integrated optimized source separately. Pass
`--compare-bin-dir DIR --compare-label baseline-564381d`; arm order alternates
by independent round. Labels are caller-supplied, not verified binary identity.
Record both source commits/build commands once alongside the campaign. The runner
records current source commit, dirty status, toolchain and host once, without
per-buffer hashes. Old speed/memory CSVs remain untouched.

Individual cells:

```
bench/target/release/bench-v2 latency --case stateful_branch --rows 10000 --repeats 7 --chunk 256 --delay-us 0
bench/target/release/bench-v2 concurrent --rows 10000 --requests 32 --concurrency 8 --mixed true --delay-us 1000
bench/target/release/memory-v2 --engine native --tasks 8 --rows 10000 --code unique
bench/target/release/memory-v2 --engine monty --tasks 8 --rows 10000 --cancel true
bench/target/release/memory-v2 --engine native --tasks 8 --bytes 10
```

## Timer and scheduling boundaries

- `cold_source_to_result`: source read, parser/compile, fresh input production and
  import, host transfer, execution, two detached operations and request cleanup.
  OS file cache is not flushed. Process/toolchain startup is outside this timer.
- `prepared_request`: required native Rc/Monty runner clone, fresh per-request
  input, dispatch, execution, effects and request cleanup. The explicit retained
  prepared owner stays outside; there is no automatic cache. Neither input
  production nor Monty object conversion is hidden outside the request timer.
- Wall subphases: source, compile/clone, produce/import, and
  dispatch/scan/wait/cleanup. Whole-request CPU uses process user+system
  getrusage, microsecond resolution. It may quantize to zero or slightly exceed
  a short wall interval. No per-phase CPU instrumentation is on the hot path.
- Each latency engine/lifecycle has one excluded real warmup. Native/Monty order
  alternates between repetitions. Repetitions within a process are observations,
  not independent runs; `--rounds` launches fresh processes.
- Concurrent mode shares prepared code, admits at most C tasks, and generates
  fresh owned input after admission. All requests are offered as one finite
  burst. Mixed mode has one heavy request per four, with the other three at
  at most 100 rows. Queue includes initial scheduling/admission delay, service
  begins at the semaphore permit, and latency is queue plus service. Aggregate
  wall/CPU include spawning and joining. Short p99 is nearest-rank over observed
  requests, not a run-level confidence interval. Monty's scan is a blocking VM
  turn; no imaginary chunk yield or native preemption is imposed on it.
- Both arms detach each provider operation and return by oneshot. Ready means
  no timer. Finite `--delay-us` sleeps once per operation, including real Tokio
  timer/scheduler overhead; it is not a network trace or exact sleep duration.
- `cold-arrival` adds one bounded offered-load control: 64 mixed requests at
  absolute 1 ms scheduled intervals, with at most eight admitted tasks. Queue
  starts at the scheduled arrival, including reactor lateness; service includes
  source read, fresh compilation, fresh input/import, execution and cleanup.
  All future arrival descriptors are preallocated and their timers use Tokio's
  millisecond-resolution clock. Compilation is synchronous on this one reactor,
  not a parallel compiler pool. Whole-run wall includes the arrival window.
  This is a finite fixed-arrival observation, not a production service capacity
  or SLO. The original `concurrent` mode remains a prepared finite burst.
- Correctness oracle and receipt checks are outside per-request timers. JSON
  output is outside the latency and whole-concurrency timers.

The default speed executable uses the ordinary System allocator. Optional
`--features allocation-diagnostics` instruments allocation calls and must be
run as a separately labeled diagnostic round, never merged into speed results.
`memory-v2` is always diagnostic. No trace/owner observers run in speed cells.

## Memory interpretation

Every memory invocation is a fresh process. Phases are process base (including
CLI/runtime setup), engine idle, first wait, second wait, completion and release.
First wait has actual per-request, touched QuoteBuffer allocations owned by
pending provider operations. Releasing the first gate executes the real program.
At second wait native has dropped its row owner/frame; Monty's ordinary local
`quotes` remains in its VM snapshot. This compares the original user source,
without inserting a Monty-only `del quotes` optimization.
The integrated campaign also runs a separately labeled `monty-clear` source
control with `quotes = []` just before record. Pinned Monty rejects `del` with
NotImplementedError (covered by a regression test). Rebinding clears the list
but the final loop variable retains at most one row. Allocation snapshots still
include VM arenas/capacity and freed slots, so clearing logical rows need not
remove their physical or requested-allocation footprint.

Shared native code uses one retained Rc owner. MontyRun clones share module
bytecode/source Arcs while cloning session tables. Unique mode actually compiles
separate sources with different initial-total literals and verifies their
individual receipts; it is not merely different arguments to one code object.
Native private code can die before record I/O. No private-code byte total is
reported when unavailable; use allocation and physical observations for that
control. Native emitted bytes exclude JIT page rounding, relocations and CLIF.

Cancellation at first wait uses the native public cancellation handle, or drops
Monty's suspended host task. Provider operations continue, then their late result
is discarded and no record is issued. Gates have 15-second diagnostic deadlines;
the runner separately records failures and timeouts. The row is an actual
allocation observation after provider completion, not a Weak-upgrade inference.
There are deliberately no Weak row observers: a Weak can keep an Arc allocation
allocated even after all strong references are gone.

`payload_live_logical_bytes` is useful row/byte payload, not physical memory.
Quote storage is a frozen exact-length slice; `input_capacity_bytes` reports its
row storage without the Arc header. Bytes controls report the actual Vec capacity.
Global counters report requested Rust layouts, freed/allocated/reallocated bytes
and peak live requested bytes since engine idle. They include diagnostic task and
collection overhead, exclude allocator metadata/reserved bins and JIT mmap pages.
RSS is sampled current resident memory. Linux RSS/PSS is from smaps_rollup and
reported VmHWM is from /proc/self/status (including compile and sampling).
Kernel RSS accounting can lag; raw VmHWM decreased slightly in small-input
pilot snapshots. `peak_rss_bytes` therefore retains the cumulative maximum of
sampled RSS and reported VmHWM, while `kernel_vm_hwm_bytes` keeps the unmodified
kernel reading. This observed high-water mark is not a continuously traced exact
peak. getrusage ru_maxrss can inherit the Python launcher's pre-exec
peak, so the Linux probe no longer uses it. macOS uses getrusage, with no PSS in
this probe, represented by null. Allocator counters and physical readers are not
an atomic snapshot; readers/output also perturb later physical samples.

For `--bytes`, native is explicitly a touched Vec<u8> provider-data ownership
probe, not native str/bytes language execution or a task-frame estimate. It has
no compiled-code control. Monty really executes bytes + len + awaited host calls.
The 0/10/4096 length controls separate fixed representation/request overhead from
per-byte requested-layout cost. Native bytes probe release/cancel phases model
owner drop only; Quote rows provide the actual async cancellation evidence.
Neither 10B input nor 10 requested bytes implies a 10B RSS increase. Physical
slopes can be zero/negative at small scales and are preserved, not clamped.

## Executed local smoke, not the final campaign

Final raw/commands/summary/report are in `bench/results/v2-smoke-macos-final/`.
On macOS ARM64 with Rust 1.96.0 and pinned Monty 56949a4, 17/17 serial commands
completed: two workload latency cells, mixed concurrent native/Monty, independent
memory processes, shared/unique Quote code, late-load cancellation, and 0/10B
controls. Eight Rust bench tests and four summary/accounting tests passed.
An additional four latency observations with 1,000 us/op finite sleeps completed
in `io-smoke.jsonl`; the allocation feature's focused test also passed. A deliberate
1-microsecond command timeout control recorded all 17 commands as timeout, with
raw/status data preserved under ignored `bench/target/v2-timeout-control/`.
The raw identity records parent 564381d plus the working delta delivered in this
lane; it is not presented as an already integrated optimized source.

With only one independent run and two latency observations per cell, these are
smoke observations, not stable performance estimates. At 100 rows, weighted-scan
median wall times were native/Monty 444.4/220.4 us cold and 3.65/102.94 us prepared.
For four 1,000-row requests, native live requested bytes fell from 118,990 at first
wait to 55,054 at second wait; Monty rose from 292,006 to 611,622 after conversion.
Physical RSS did not fall with native requested-byte reclamation, illustrating
allocator retention. In the raw native bytes probe, fixed Vec descriptor cost was
24 requested bytes/task and the 10B payload added 10. That is not native language
task memory. Monty's first-wait fixed snapshot cost was 41,799 requested bytes/task
in this small diagnostic control. No claim is made that these fixed terms are
production minima.

Linux validation and baseline/optimized comparison remain for integration card
C. Figures and architectural conclusions must use that measured campaign; this
lane does not change owner APIs, optimize the runtime, or publish the final report.
