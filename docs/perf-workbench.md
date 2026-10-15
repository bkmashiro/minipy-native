# Resident-request performance workbench (BT)

A small test executable plus a stdlib Python driver, not another benchmark platform.
Production `src/`, dependencies, existing campaigns and default runtime are unchanged.
This is synthetic mechanism evidence on macOS, not a Monty comparison or a production
latency distribution. No Linux or Wasm execution is claimed.

## Reused pieces and scope

- `tests/tool_environment.rs`: legacy vs registered declarations and both public backends.
- `bench/src/bin/bo_request.rs` and `tools/bs_request_pair.py`: resident Host, source-cold
  full requests and fixed-binary comparisons. Unlike the historical script, this
  driver never creates a worktree or switches source during measurement.
- `tests/http_tool.rs` / `tests/http_collect_cost.rs`: real pooled loopback HTTP.
- Existing opt-in `compiler-profile` supplies native lowering/backend attribution.
  No new production instrumentation or allocator is added.

The `tests/perf_workbench.rs` executable defaults to four cases:

| Case | Work inside each request |
| --- | --- |
| scalar | new literal, integer addition and print |
| quote | construct/import 256 typed Quotes, checked weighted scan, print 65280 |
| ready | ready Tool, JSON alias, string contains and string output |
| loopback | same JSON source over real pooled HTTP/1.1 on 127.0.0.1 |

BV additionally provides opt-in `scalar_inputs` and `ready_inputs` with two
injected i64 values (7, 9), both consumed by the source. Default case selection
is unchanged. The private schema/frontend preparation diagnostic and bounded
A/A decision are in [BV evidence](../evidence/bv-input-gate/README.md); they add
no production optimization or public schema API.

Both tape and actual native code execute and assert exact stdout/status. Every
iteration changes a source literal and reparses/prepares it; there is no prepared
unit reuse or source/result cache. There are 16 immutable Tool declarations,
including 15 unused declarations, matching the known environment-reuse experiment.
The ready JSON producer returns static Bytes; Quote producer construction/import
is inside request timing. The loopback server runs on the same current-thread
Tokio I/O-enabled runtime as the client. This measures server work and local
scheduling too, not remote HTTP, TLS or a chunking optimization.

## Boundaries

Startup is separate: runtime creation, declaration/environment registration,
Provider/HTTP service construction, listener bind and fixture setup. Both legacy
and registered controls construct the environment at startup. It is not a fair
startup comparison of those entrypoints. Process launch and dynamic linking are
outside both reported intervals. The first request opens the pooled connection.

Formal request timing is one `Instant` interval around preparation, execution,
code release and outcome/output release. Tiny exact-status/stdout assertions are
inside that interval so output can be checked before it is freed. Source string
and expected-output construction are outside; caller-owned source remains live
until the iteration ends. Host/environment/runtime are resident resources and
remain live. No per-phase timers, compiler-profile hooks, global counting allocator
or retained CLIF inventory are active in formal builds. Reporting and JSON row
construction happen after the timer; rows are printed after the process's batch.

Each process emits one setup row, sample 0 (warmup), then samples 1..N. The reducer
excludes setup and sample 0 by structure and validated sample IDs, never by a
manually transcribed row index. It reports per-process medians/min/max, then the
median of process medians for each arm. B/A is descriptive, not a significance
claim. A/A uses exactly the same binary AND entrypoint in both arms. ABBA slot
order is per cell across independent processes, repeated in the requested rounds.
There is no automatic speed pass/fail threshold or rerun-until-win policy.

A process panic/nonzero exit, timeout, malformed/missing samples or binary mutation
is a failed process. Partial rows are preserved but not timed. Missing processes,
failed processes and missing end records make the entire run incomplete and
suppress every B/A delta. Duplicate/unexpected processes or samples are errors.
Raw is gzip JSONL, flushed per process and never overwritten. Abrupt OS termination
can leave an invalid gzip stream; reduction then fails rather than assuming success.

## CLI

Run from the repository. Build/run operations take
`~/.hermes/cache/scratch/minipy-perf-build.lock` using `fcntl.flock` and serialize
with other experiments. Build checks the mounted external target symlink, uses
`CARGO_BUILD_JOBS=1`, Rust 1.96.0, `--offline --locked --release`, and copies only
the one executable into the specified scratch path. It writes a `.json` sidecar
with binary, source and toolchain identities. No new target or worktree is made.

```sh
S="$HOME/.hermes/cache/scratch"
python3 tools/perf_workbench.py build --output "$S/bt-formal-v2"
python3 tools/perf_workbench.py run --baseline "$S/bt-formal-v2" --aa \
  --reps 24 --output evidence/perf-workbench/aa.jsonl.gz
python3 tools/perf_workbench.py run --baseline "$S/bt-formal-v2" \
  --candidate "$S/bt-formal-v2" --baseline-entry legacy \
  --candidate-entry registered --cases scalar ready --rounds 2 --reps 32 \
  --output evidence/perf-workbench/registered-control.jsonl.gz
python3 tools/perf_workbench.py run --baseline "$S/bt-formal-v2" --aa \
  --cases scalar ready --rounds 2 --reps 32 \
  --output evidence/perf-workbench/aa-control.jsonl.gz
python3 tools/perf_workbench.py reduce evidence/perf-workbench/aa.jsonl.gz
python3 -m unittest discover -s tools -p test_perf_workbench.py -v
```

Paths above are this run's commands. Raw exists already, so reproduction must use
fresh output names; scratch executables are deliberately removed after completion.
For a future production candidate, build A and B separately with this identical
adapter, preserving both snapshots and sidecars, then use `--baseline A --candidate B`
with matching entrypoints. The runner does not edit/build either arm while timing.
It rejects mismatched adapter source hashes and diagnostic/formal identities, and
rehashes each executable before and after execution under the shared lock.

## Bounded real validation

Source base `485271a801588712c326c2113bfdec8ed44c6426`, Apple M4/macOS 26.4.1,
rustc `1.96.0 (ac68faa20 2026-05-25)`. Exact adapter SHA, executable SHA, core source
SHA and command identity are embedded in each raw manifest. Later tool-only
reducer improvements do not alter those measured executables.

| Raw | Processes | Measured requests | Purpose |
| --- | ---: | ---: | --- |
| smoke-v2.jsonl.gz | 32 | 64 | all four cases × both backends, 2 requests/process |
| aa.jsonl.gz | 32 | 768 | initial A/A, all cases, 24 requests/process |
| registered-control.jsonl.gz | 32 | 1024 | two-round known legacy/registered control |
| aa-control.jsonl.gz | 32 | 1024 | matched shape/count A/A after the known control |
| diagnostic.jsonl.gz | 4 | 48 | separate profiled native ready requests |

All five completed with no task/process failures or missing samples. Initial
`smoke.jsonl.gz` is also preserved: all 32 processes exited 0, but a parser bug
missed libtest's first line (`test ... PW ...`), yielding 32 failed protocol cells
and no speed deltas. `smoke-v2` repairs extraction. Its binary predates a diagnostic-
only typing/warning fix; the three formal control runs use the final adapter.
This is a real failure-accounting record, not counted as native task failure.

Initial A/A B/A ranged **0.373–1.015**, with strong drift in short tape processes.
Do not infer a speedup from that run. The bounded, matched-shape second A/A and
known registered-environment control give these medians of process medians:

| Backend/case | A/A B/A | Legacy A ns | Registered B ns | Control B/A |
| --- | ---: | ---: | ---: | ---: |
| tape scalar | 0.976 | 4791.75 | 2541.75 | 0.530 |
| tape ready | 1.054 | 13552 | 11187.5 | 0.826 |
| native scalar | 0.993 | 43593.5 | 40750 | 0.935 |
| native ready | 0.992 | 83447.75 | 80062.75 | 0.959 |

The tool reproduces the expected direction of avoiding per-source validation of
16 registered declarations. The tape effect is larger than this matched A/A
variation. Separate ordered A/A runs do not prove a noise bound or a stable native
4–7% improvement, especially given the first run's drift. No runtime optimization
is proposed or retained by BT, and no full campaign was rerun.

## Independent diagnostic

```sh
python3 tools/perf_workbench.py build --diagnostic --output "$S/bt-diagnostic"
python3 tools/perf_workbench.py run --baseline "$S/bt-diagnostic" --aa \
  --diagnostic --cases ready --backends native --reps 12 \
  --output evidence/perf-workbench/diagnostic.jsonl.gz
python3 tools/perf_workbench.py reduce evidence/perf-workbench/diagnostic.jsonl.gz
```

This separate executable enables existing `compiler-profile`/Cranelift timing
hooks and test-only phase timers. It is rejected in a formal run; its reducer
intentionally emits no speed ratio. Across 48 non-warmup requests, pooled medians
are total **214.563 µs**, prepare **202.188 µs**, execute including unit release
**11.813 µs**, Cranelift backend **158.834 µs**. The median per-request backend /
prepare fraction is **78.7%**. Medians are not additive. This localizes the biggest
instrumented new-source cost to native preparation/backend, not JSON execution.
It does not quantify a removable optimization or establish production CPU shares.

The existing profile does not instrument frontend/parse/type counters on the
ToolEnvironment entrypoint: their recorded zeros are **unmeasured**, not free
parsing. The outer prepare timer includes that work. Lowering/setup/finalize and
compiler temporary cleanup counters are available. The execution phase includes
runtime unit destruction; output release is separately timed. Diagnostic timers
and hooks substantially change cost, so none of these values are pooled with
formal requests. Allocation counts, waiting-state retained bytes and system-wide
sampling are not measured in this lane; existing specialized probes remain the
right tools if a downstream hypothesis needs them.

## Checks and decision

Eight reducer contract tests cover setup/warmup exclusion, failed/partial/missing
processes, duplicate processes/samples, diagnostic mismatch, invalid durations and
missing end records. Strict release Clippy passed for this test in both formal and
diagnostic feature modes. Eleven related runtime tests passed (ToolEnvironment,
HTTP and string contains); the existing environment timing test remained ignored.
Default rustfmt was used because rustfmt is absent from the pinned 1.96 toolchain;
no component or dependency was downloaded.

Keep the small CLI, adapter, tests and compressed raw. The next BV lane can use
fixed snapshots for at most two source-backed candidates, run diagnosis separately,
and stop on noise or negative evidence. Do not add a scheduler, metadata service,
cache or a new benchmark matrix based on this tool validation.
