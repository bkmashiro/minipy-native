# BY: explicit Ready Tool dispatch, bounded negative experiment

Decision: **revert the production/API prototype and defer Ready dispatch**.
There is a feasible additive API, so this is an executed experiment, not an API
impossibility claim. Synchronous completion eliminates real work, but the first
bounded implementation widens both execution futures by 216 B and does not show
consistent fresh-source request benefit. No scheduler redesign or larger campaign.
Base: `03a39aff76a49b733cedbc36de432f8359cd3504`, macOS 26.4.1 ARM64, Rust 1.96.0.
Final `src/`, manifests and workbench adapter match the base exactly.

## Compatibility gate and isolated mechanism

The existing `Provider` returns opaque `impl Future`. It does not expose whether
an operation is synchronously complete. Polling it inline to discover Pending
would submit work before the detached owner exists, change panic/cancellation
behavior and require migrating a borrowed future. That route is not implemented.
BE's JoinHandle substitution only changed completion notification; it is still a
negative and its detached+yields timing is not attributed wholly to a channel.

The archived additive API is `Provider::begin_tool(id, owned ToolArguments)` ->
`ToolDispatch::Ready(Result<Reply, TaskError>) | Deferred(ToolArguments)`.
Default returns Deferred. Existing Provider implementations compile unchanged.
Only bound Tool calls change, identically in native/tape; Quote load/record,
HttpProvider, I/O driver, semaphore admission, transport and timeout stay intact.
This is a public API experiment kept as a patch, **not** a new main-line API.

Contracts and ordering:

- Argument evaluation/validation stays in original order before authorization.
  Cancellation, capability and call-budget debit run before begin_tool.
- Ready is explicit bounded, nonblocking synchronous completion, with an owned
  result, no pending future, task, borrowed parameter or permit to keep alive.
  Provider is trusted TCB; the type cannot prove it refrains from hidden work.
- Deferred must not submit an effect. Its owned arguments pass exactly once into
  the original detached spawn/oneshot call_bound_tool path. No speculative poll,
  duplicate invocation or executor extension. Default HTTP remains Deferred.
- Synchronous panic is caught with catch_unwind and mapped to the same
  `Provider("operation lost")` task error as a panicked detached operation.
  This assumes the existing unwind panic strategy, not panic=abort isolation.
- Every Ready result/error/panic yields once, then checks cancellation before
  consumption. Thus Ready call chains remain interruptible between calls and a
  concurrent cancellation wins over a finished reply, as on receive(). Exact
  relative scheduling of Ready effects versus other tasks is not promised.
- JSON reply kind, buffer budget, root liveness, output and scan fuel stay in the
  existing consumers. No bytes or authority check is removed. Cancelled Deferred
  operations still own parameters/permits until actual completion and cannot
  resurrect cancelled task state.

Archived `tests/by_dispatch.rs` covers both engines: ready success/error/panic,
wrong reply kind, cancellation during ready completion, no entry dispatch after
cancellation/permission/budget rejection, one effect exactly, and cancellation by
an actual sibling after the first of eight ready calls yields. Existing full
suites also exercise delayed StringValue owners, late completion and real HTTP.
An initial 64-call test was rejected at frontend MAX_LOCALS; reduced to eight
calls within existing admission limits, without changing production limits.
The initial failed verification is retained as `candidate-verification.json`.
`candidate-verification-fixed.json` is the passing candidate verification.

## Fixed binaries, whole fresh-source requests

A/B and A/A each have **64 successful processes / 3072 measured requests**.
Two ABBA rounds, 48 measured requests/process, both engines and four cases.
Status and exact stdout are asserted for every real request. A/A is exactly the
baseline executable in both arms, not separately rebuilt binaries.

Both arms contain the same dormant additive API and identical Host opt-in in the
adapter; A retains original production dispatch and never calls begin_tool.
B changes only runtime/interpreter dispatch. Thus these are not a comparison
between two different Host reply implementations: the ready responses are the
same static JSON bytes. Baseline API/adapter patch is separately archived.

The BT interval includes each changed source's prepare/compile, actual execution,
Tool handoff, output assertions and code/outcome cleanup. Source construction is
outside. Resident Host/ToolEnvironment/runtime/listener setup is reported
separately (median startup A/B archive 73.437 us, A/A 72.625 us), not counted as
request savings. No prepared-unit/cache hit assumption, diagnostic timers,
allocator or compiler instrumentation in these formal executables.

| backend | case | A ns | B ns | A/B B:A | A/A B:A |
|---|---|---:|---:|---:|---:|
| tape | ready | 10427.25 | 9874.75 | 0.9470 | 1.0072 |
| tape | scalar | 2396.00 | 2260.25 | 0.9433 | 1.0232 |
| tape | Quote | 11333.50 | 11520.75 | 1.0165 | 0.9972 |
| tape | loopback | 91760.50 | 91250.00 | 0.9944 | 0.9979 |
| native | ready | 82728.75 | 83603.75 | 1.0106 | 1.0127 |
| native | scalar | 41093.75 | 40906.50 | 0.9954 | 1.0050 |
| native | Quote | 111979.25 | 110854.50 | 0.9900 | 0.9964 |
| native | loopback | 172479.00 | 170031.50 | 0.9858 | 1.0018 |

Arm values are medians of process medians, not pooled samples. The unchanged
scalar tape control improves slightly more than the Ready cell. Raw preserves
large early drift: tape ready A process medians 24.2705, 10.3545, 10.5000,
10.0835 us, B 23.7295, 9.7080, 9.9790, 9.7705 us. Native Ready does not improve;
HTTP differences are controls, not a claimed I/O win. Separate A/A cannot supply
a statistical noise bound or remove process drift. No consistent causal E2E win
is established and this campaign stops, rather than rerunning until favorable.

## Separate allocation and future layout diagnostic

`baseline-diagnostic.json` / `candidate-diagnostic.json` use an ignored libtest
with a counting System allocator, prepared units reused and one call per
execution. They are not source-cold timing. Four samples after sample 0 per
backend/mode, serial current-thread I/O-enabled runtime; report medians:

| metric, numeric one-call fixture | A | B |
|---|---:|---:|
| tape execution future size, both modes | 928 B | 1144 B |
| native execution future size, both modes | 664 B | 880 B |
| tape explicit-ready allocations / requested bytes | 4 / 548 B | 2 / 68 B |
| native explicit-ready allocations / requested bytes | 5 / 868 B | 3 / 388 B |
| tape Deferred allocations / requested bytes | 5 / 612 B | 5 / 612 B |
| native Deferred allocations / requested bytes | 5 / 868 B | 5 / 868 B |

The explicit-ready diagnostic removes two allocation calls / 480 requested bytes
in each backend, consistent with eliminating spawn/task and oneshot bookkeeping.
These are allocator-requested totals, not exact heap occupancy or a universal
per-call constant; executor allocation reuse affects counts (including A's mode
sequence). No payload-copy savings, RSS or waited-task density improvement is
claimed. B's concrete execution future grows **216 B even in Deferred mode**.
That layout cost, plus no consistent request gain, outweighs retaining this API
prototype. The diagnostic has no real long-wait heap census. Full verification
is evidence of behavior, not evidence that future growth has no density impact.

## Reproduce

All builds/measurements use the existing mounted external target symlink and
shared `fcntl.flock` lock via BT/by_probe, `CARGO_BUILD_JOBS=1`, Rust 1.96.0,
`--offline --locked --release`. No new worktree, target, dependency or download.
Use fresh output names. The committed patches apply to the final production tree:

```
S="$HOME/.hermes/cache/scratch"
git apply evidence/by-ready/candidate.patch
python3 tools/perf_workbench.py build --output "$S/by-b-new"
python3 tools/by_probe.py evidence/by-ready/candidate-diagnostic-new.json
python3 tools/by_probe.py evidence/by-ready/candidate-verification-new.json --verify
git apply -R evidence/by-ready/dispatch.patch
python3 tools/perf_workbench.py build --output "$S/by-a-new"
python3 tools/by_probe.py evidence/by-ready/baseline-diagnostic-new.json
git apply evidence/by-ready/dispatch.patch
python3 tools/perf_workbench.py run --baseline "$S/by-a-new" --candidate "$S/by-b-new" --cases scalar ready quote loopback --rounds 2 --reps 48 --output evidence/by-ready/ab-new.jsonl.gz
python3 tools/perf_workbench.py run --baseline "$S/by-a-new" --aa --cases scalar ready quote loopback --rounds 2 --reps 48 --output evidence/by-ready/aa-new.jsonl.gz
git apply -R evidence/by-ready/candidate.patch
python3 tools/by_ready_reduce.py
```

Original A was built before the diagnostic test was added; this does not change
its workbench executable. Formal manifests carry full source/toolchain/binary and
adapter identities. Candidate diagnostic predates the admission-only test fix
and rustfmt changes, which do not affect its executed diagnostic. Helper outputs
also record source hashes and diagnostic test SHA. For new timing archives run
`tools/perf_workbench.py reduce <new raw>`; BY reducer recomputes originals.
Temporary fixed binaries are removed after delivery; shared target is retained.

## Verification, decision and next lane

Candidate and final-restored default and tool-json/debug-runtime full release
suites pass on Rust 1.96.0. Existing detached string/HTTP/authority/lifetime tests
are not rewritten to force Ready. Nine BT Python contract tests, rustfmt,
archived patch applicability and diff checks pass. Diagnostic and verification
outputs retain actual commands, exit codes, stdout/stderr and source identity.
No Linux, Wasm, Monty, real remote HTTP, production workload or p99 claim.

Keep evidence and the reproducible prototype/regressions only. Revert begin_tool,
ToolDispatch, dispatch helper and Host opt-in from production and normal tests.
Defer API adoption and a same-job async continuation design; do not revive BE by
renaming its container substitution. The pre-created BZ lane should examine
large-root/small-string retention across a genuinely asynchronous call, where
backing bytes and ownership lifetimes may offer larger value without adding an
execution topology. BY does not pre-implement that independent experiment.
