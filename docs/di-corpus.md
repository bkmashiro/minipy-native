# DI: controlled corpus admission and lifecycle runner

This is a small Host replay adapter, not a Python executor or a coverage/performance
campaign. Only `fixtures/di/smoke.jsonl` is synthetic public data. Real source,
provenance, expected output and results belong outside Git, including failed
attempts. The runner never interprets shell commands, opens requested files,
sends messages or makes HTTP calls. A Tool named `terminal` is just a declared
replay name. No production runtime, syntax, dependency or authorization changes.

## Run

From the repository, with the existing mounted external `target`:

```sh
python3 tools/di_corpus.py --input fixtures/di/smoke.jsonl --mode timing --output bench/results/di/timing.jsonl
python3 tools/di_corpus.py --input fixtures/di/smoke.jsonl --mode diagnostic --output bench/results/di/memory.jsonl
python3 tools/di_corpus_test.py
```

DL adds one process/campaign-wide Host setting, `--stdout-bytes N`, forwarded
to the existing `ScriptLimits.stdout_bytes` on both backends and in both modes.
It defaults to 262144 (256 KiB), an engineering choice for short-document
scripts, not a measured corpus requirement. Accepted range is 0..16777216
(16 MiB harness ceiling). Zero disallows nonempty output. Invalid configuration
fails before corpus execution. There is no per-case override. Both the manifest
and every prepared/executed result report `policy.stdout_bytes`, the resolved
byte cap. Direct Rust invocation accepts the same optional flag and default.

The library `ScriptLimits::default()` remains 16 KiB: changing it would alter
existing embedded Hosts, the bounded browser debugger and historical benchmarks.
Document-oriented Hosts can use the same explicit override without a new public
runtime API. To reproduce pre-DL DI/DJ/DK output policy, pass
`--stdout-bytes 16384`; old result files retain their historical meaning and are
never overwritten. Freeze one policy before generation, disclose it in prompts,
and keep it uniform across tasks, arms and repairs. Do not size a case's budget
from its expected output or use a higher quota to reinterpret a historical score.

Accounting remains cumulative UTF-8 bytes including each print newline. The
existing runtime checks a complete print before appending it, retains earlier
stdout on error, and preserves permission/cancellation and no-runtime-fallback
rules. Storage grows only as output is produced; the cap is not an allocation
request. JSON token, source, literal, scan and buffer limits remain independent.

The executor's diagnostic is `error: {category, reason, location}`, not
`admission`. A repair adapter should forward these actual compiler fields for
preparation errors. Runtime locations remain null; runtime Provider messages,
reply bodies, actual/expected stdout and expected submissions must not be sent
back as hidden oracle feedback. A correctness mismatch can disclose field names
without values. Retain the first response and allow at most one repair.

The wrapper checks the mount, takes the shared `minipy-perf-build.lock`, uses
`CARGO_BUILD_JOBS=1 RUSTUP_AUTO_INSTALL=0 cargo +1.96.0 --offline --locked`, and
reuses the existing target. No new worktree/target or downloads. It rejects an
existing output path. Each output has a source/toolchain/binary/input identity,
one runtime setup record per backend, then one result per input case/backend.
A nonzero runner exit or timeout is retained as a process-error record, not a
successful case. Build failures remain visible on stderr. Limits of 256 cases,
64 MiB input, 512 replies/case and 1000 ms/wait are harness engineering choices,
not language limits. The Rust example also runs directly on JSONL stdin, but
use the wrapper for identities, serialization, inventory and mount checks.

## JSONL v1 contract

One JSON object per case. Required `case_id` and `source` strings. `inputs`,
`tools`, `replies` default to empty arrays. Arbitrary top-level private metadata
is retained in the input and ignored by the Rust executor. Unknown fields in
ABI/argument/result/reply records are rejected as `fixture_error`.

- `inputs`: `{name, type: "i64" | "bool", value: i64}`. Bool values are 0/1;
  runtime input validation, including invalid bool, stays unchanged.
- `tools`: `{id, name, arguments, result}`. IDs >=2, unique non-reserved names.
  Arguments are `{name, type: "i64" | "string"}`. The existing ABI accepts at
  most two numeric arguments, or exactly one string argument. Names and result
  schemas are validated by the real frontend, not a permissive adapter.
- Result is exactly one tagged object:
  - `{kind: "i64"}`
  - `{kind: "json", leaves: [{path: [string], type: "i64" | "string"}]}`
  - `{kind: "object_array", path: [string], i64_fields: [string]}`
  JSON typed leaves and an object-array schema cannot be combined in one Tool.
- `replies`: ordered `{tool_id, arguments: [JSON integer | string], wait_ms?}`
  plus exactly one of `i64: integer`, `json: raw_JSON_text_string`, or
  `error: string`. Raw JSON is not eagerly parsed into an object tree; invalid
  payloads reach the actual runtime's lazy error path. `wait_ms` defaults to 0.
  Reply arguments are in declaration order. String arguments use a one-element
  array. Reply bodies move from the per-request queue into runtime ownership.
- Legacy `record_total` has ID 1 and two i64 arguments; its replay is controlled
  identically. Legacy `load_quotes` ID 0 is unavailable in v1: an actual Provider
  entry is logged and returns Permission. No Quote reply ABI is invented.
- `expected`, optional, is exactly `{stdout: string, status: string,
  calls: [{tool_id, arguments}]}`. `status` is `ok`, a task error category, or an
  admission category. Expected extra fields fail exact equality. Omitted
  expected gives `correct: null`, not success. With both completeness flags true,
  this expected target can describe
  desired semantics even when the original source is Unsupported, yielding
  `correct: false`. It need not endorse rejection as a task-level solution.
- `contract_complete` and `oracle_complete` are corpus-author flags. An unknown
  or false Host contract never contributes whole-program binding coverage.
  `correct` is boolean ONLY when BOTH flags are explicitly true and `expected`
  is present and non-null; otherwise it is null, even if supplied expected output
  matches. This gate applies in both the Rust adapter and Python decoration.
  Flags are optional booleans (null means unknown); non-booleans are fixture errors.
  The runner does not manufacture missing contracts/oracles. Restrict a later
  correctness pilot to cases with BOTH flags true, preserving all other cases
  in the sampling/obstacle denominator. This runner does not launch that pilot,
  rewrite sources or call a model.

Task status categories: Overflow, Budget, Cancelled, Permission, Buffer, Input,
Provider, Json. Admission categories: Unsupported, Syntax, Type, Schema,
Resource, Backend. Admission locations are the actual one-based line/UTF-8 byte
column, or null when the compiler supplies none. Runtime source locations are
unknown/null; the runner does not infer them from an error message.

## Routing, correctness and binding

The adapter retains CY's prepare-entire-source-before-provider contract, extended
through the existing declaration-aware entry points. Unsupported alone produces
`fallback_stub`: no alternative executor is invoked. Other admission failures
are rejected with their own category. Every runtime outcome, including partial
stdout and errors, is `executed` and never retried/replayed. Every backend gets a
new independent controlled Host and reply queue.

`actual.calls` records Provider handler entries (polled submissions), with exact
ID and decoded arguments in order, including a mismatching or failing call.
An id/argument/sequence mismatch returns a Provider error and sets
`replay_matched: false`. Unused replies are allowed, because branches, early
errors and rejection can legitimately consume only a prefix. The expected
ordered call trace separately enforces which submissions should occur. Permission
checks that deny entry do not count as submissions. Schema does not confer real
Tool authority; this Host allows only declared IDs and its controlled legacy
record operation.

`static_binding.source_sites` is an untimed, whole-source CPython AST inventory:
all `tools.<name>(...)` call expressions; all subscript expressions, non-call
attribute accesses, and `.get(...)` final selectors. Chained selectors count
one per syntactic selector, not one per runtime access or unique declared path.
It includes unreachable sites and unsupported tails. Invalid Python syntax
makes the inventory null. CPython parsing is an inventory convenience; Rust
admission remains authoritative.

`whole_program_bound_sites` equals that inventory ONLY after full preparation
succeeds AND `contract_complete` is true; otherwise null. This describes admitted
syntactic sites against the frozen Host ABI, not instrumented compiler slot
counts, actual executed field reads, useful task success or production coverage.
No prefix-parse/partially bound numerator is reported. Runtime failure does not
undo successful static binding, but cannot become correctness success.

## Timing boundary

Like BT's resident request adapter, a current-thread Tokio runtime is initialized
once per backend process, then a scalar warmup is prepared/executed/dropped.
Every corpus line gets fresh source allocation, real prepare, real native/tape
execution and unit teardown. No source cache, no synthetic business kernel.
The setup record measures runtime construction, excluding fixture decoding.

Timing mode uses no runtime Trace, owner census, allocation counter or compiler
profiling. It still includes the required controlled Host argument logging,
replay matching and wait clocks. Its intervals are:

- `setup_ns`: per-request reply copying, declarations/input construction and Host
  registration. This is NOT silently treated as one-time service setup.
- `fresh_source_prepare_execute_cleanup_ns`: source allocation through prepare,
  execution, outcome/code teardown and source drop. Retaining a reporting copy
  of stdout is included. Failed preparation has zero execute/unit-cleanup time.
- `request_including_setup_cleanup_ns`: includes the preceding per-request setup,
  source interval, reporting trace copies and Host/declaration/input teardown.
  The original immutable decoded case remains resident as the input corpus.
- `prepare_ns`, `execute_ns`, `prepared_cleanup_ns`: observed subintervals, not a
  claim that their sum includes all request work. Execution includes Tool waits.
- `tool_waits`: each explicitly gated replay's requested delay and observed
  timer-await interval, indexed by zero-based submission. Ready Tools have no
  gated wait record. Timer scheduling delay is included. Do not call the
  difference between total time and timer intervals pure compute time.

JSONL reading/decoding, CPython inventory, expected equality, final report JSON
serialization/destruction, process launch and binary build are outside request
intervals. Reporting copies and matching are harness costs, not free production
operations. These are one-shot smoke phase observations, not strong-baseline
performance results, latency distributions, or evidence of speedup. No Monty
comparison or campaign is run on this card.

## Separate ownership diagnostic

Diagnostic mode sets each result's `timing` to null. It prepares/runs separately,
turning on existing runtime Trace and a Host-only `Bytes::from_owner` backing
observer. Its added locks/maps/allocations never run in timing mode.

- Tape preparation reports the existing `prepared_bytes()` logical owned-byte
  measure (including capacities, excluding allocator rounding/Rc header).
  Native emitted code bytes are available; native metadata is unknown/null.
- Owner phases are actual Provider `before_submission`, gated `tool_wait_start`
  / `tool_wait_end`, JSON `reply_handoff`, `execution_end`, `prepared_dropped`.
  A fresh Vec backs each handed-off JSON reply. IDs count each allocation once,
  and its destructor removes it only after the last Bytes reference dies.
  `unique_json_owner_backing_bytes` sums backing Vec capacities. This observer
  retains no owner. A source JSON String still in the decoded fixture or pending
  replay queue is Host setup storage and is NOT claimed as runtime owner backing.
- These observations cover raw handed-off JSON owners, not decoded string
  caches, task arrays, parser scratch, futures, Host queue storage or total heap.
  `task_state_bytes`/`total_task_state_bytes` remain null. No guessed private
  layout or `size_of(Future)` is presented as total task memory.
- Existing runtime events separately report `reported_frame_bytes` and
  `runtime_buffer_accounting_bytes`. They are partial logical runtime budget
  accounting (potentially including decoded strings/cursor metadata), NOT the
  unique raw backing census. Zero frame bytes means this event did not report
  a frame, not a zero-sized task. Events and owner phases are separate sequences;
  there is no invented one-to-one phase alignment or transient peak estimate.
- `rss_pss` is null. None of these logical counters are RSS/PSS or allocator
  allocation/retained totals. A final zero owner census proves release of this
  Host's handed-off raw owners only.

## Validation scope

Ten synthetic fixture cases exercise scalar, JSON wait, string get/default/Tool
sink, complete-source Unsupported/Type/Syntax/Schema rejection, overflow after a
Tool side effect, provider error, and a bounded object array. Targeted Rust tests
run both backends in both modes and add resource rejection, replay mismatch,
wrong JSON type, numeric two-argument ABI and strict fixture validation. Python
unit tests protect denominator definitions and unknown/incomplete contract
handling. No private case, model generation, corpus campaign, Linux/Wasm run,
new target or production semantic expansion is part of DI.

DL adds synthetic quota tests for exact/one-byte-short multibyte print boundaries,
zero/custom limits, prior output and later-effect suppression, cumulative document
output above 16 KiB and at the new cap, permission/pre-cancellation, and the
existing assignment boundary. Same-type i64/bool reassignment is supported.
JSON roots/views/string-leaf aliases are single static bindings; rebinding would
change owner/alias semantics and is deliberately not enabled by this repair.
Use distinct root names for sequential JSON Tool replies. JSON dot/subscript
selectors share that boundary. Preserve decoded Tool string arguments exactly,
including all source/JSON escape layers. The versioned private v3 guide and
repair regressions stay outside Git; DL performs no model generation or campaign.
