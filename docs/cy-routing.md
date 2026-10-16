# CY: whole-source admission and Host routing

`examples/cy_route.rs` demonstrates the existing library API on both native and
tape backends. Run `cargo +1.96.0 run --offline --locked --release --example cy_route`.
The Host selects the backend, declares scalar inputs, and supplies its provider,
limits and cancellation. The entire source is prepared before execution. The
example has no external executor, subprocess, source rewrite, source cache, or retry.

## Boundary

- `RouteOutcome::Unsupported(CompileError)`: preparation returned the structured
  `Unsupported` kind. No source statement ran, no private stdout was produced,
  and no Tool was dispatched. The caller can choose a separately authorized
  general executor. The test stub only records that choice and executes nothing.
- `Rejected(CompileError)`: Syntax, Type, Schema, Resource or Backend admission
  failure. The example does not offer fallback for these failures.
- `Executed(ScriptOutcome)`: execution started, including failed input checks,
  cancellation, permission, budget, overflow and provider errors. Preserve the
  exact status and stdout prefix. Never replay any of this source on another
  executor, even when no Tool is observed to have completed.

`Unsupported` identifies this compiler's rejection category, not proof that a
program is otherwise valid in a general Python implementation. The Host still
owns authorization, cancellation, resource admission and error handling for any
external executor. Fast-path i64 arithmetic is checked; generic arbitrary-precision
Python is not an equivalent retry path after overflow. A declaration is not
permission. Submitted detached effects are not rolled back by cancellation.

## Minimal API change

`AdmissionKind::Resource` separates finite preparation limits from unsupported
language forms. Existing `CompileError` reason, location and priority are retained.
It covers source bytes, preparser/AST depth and nodes, scalar/buffer/JSON binding
counts, and bounded literal/parts limits. Invalid Host schemas remain `Schema`;
integer type-range failures remain `Type`. Execution budgets remain `TaskError`.
This is an additive public enum variant: exhaustive downstream matches must handle
it. The existing BI diagnostic example was updated. There is no new production
routing layer and no hot-path check or allocation.

## Complete-script matrix

The ten unchanged `fixtures/cv/*.py` are synthetic engineering cases, not a
production workload distribution. Under the declared in-memory Host in
`tests/string_equality.rs`, real native and tape execution checks stdout and exact
Tool arguments/order. CY reran that test, including the alternate 07/08 branches.

| CV | Complete original script | Remaining first gap |
| --- | --- | --- |
| 01 status summary | rejected | direct JSON-i64 leaf in f-string |
| 02 few JSON fields | runs | — |
| 03 path then call | runs | — |
| 04 first matching result | rejected | JSON array representation/iteration, then break |
| 05 missing/default | rejected | get/default semantics |
| 06 multiple calls/branch | runs | — |
| 07 exact route | runs | — |
| 08 nonempty message | runs | — |
| 09 prefix route | rejected | startswith |
| 10 alias contains summary | runs | — |

Six complete originals run, unchanged from CX. CY makes their admission boundary
usable by a Host; it unlocks no additional language syntax. The next small
candidate is direct JSON-i64 f-string consumption for 01, but it is not implemented
or scheduled by this task. Arrays/defaults/prefix operations remain independent
gaps. Stop this phase here rather than expanding toward full Python.

DA update: the CY matrix above records the CY snapshot. Direct JSON-i64 f-string
consumption is now implemented and CV01 runs unchanged in both backends, including
its non-ready branch. The current count is seven synthetic complete originals;
04/05/09 retain the listed gaps. See `tests/json_fstring.rs` and
`bench/results/da/README.md`. This count is not a production coverage percentage.

DB update: CV09 now also runs its unchanged complete source, both draft/final
branches on native and tape. The current synthetic total is eight originals;
only 04/05 remain unsupported. DA/CY counts above are historical snapshots.
See `bench/results/db/README.md`. No production coverage percentage follows.

DF update: CV05 now runs unchanged on both backends with declared-string literal
get/default. Nine synthetic originals run, with only CV04 arrays/iteration still
unsupported. Runtime required-parent/value-type/JSON failures remain executed errors,
never routing triggers. Earlier counts are historical. See [df-get.md](df-get.md)
for the missing-only validation policy and public enum migration.

DG update: CV04 now runs unchanged using its explicit object-array Host schema
on native and tape. The current synthetic total is ten of ten complete originals;
the earlier counts are historical snapshots, not production coverage. Runtime
array/type/budget errors remain task failures, not routing triggers. Unsupported
general collections still reject before effects. See [dg-arrays.md](dg-arrays.md).

## Verification and bounded timing

`python3 tools/cy_check.py` uses the existing external target, shared fcntl lock,
one build job and Rust 1.96.0 offline/locked. It records source hashes, commands,
toolchain and logs in `bench/results/cy/`:

- Default full release suite: 84 passed, 3 existing performance tests ignored.
- tool-json full release suite: 145 passed, 13 existing performance tests ignored.
- tool-json + debug-runtime routing/equality: 11 passed.
- New routing tests: 4 default / 5 tool-json tests, each exercises both backends.
  Fresh success, unsupported tail and dead branch before any Tool, malformed
  source/type/schema/resource rejection, runtime errors after an earlier effect,
  partial stdout, pre-cancel and cancellation after detached submission are checked.
- Targeted library/new examples/new test Clippy passes with the existing CT
  `type_complexity` allowed. This is not a clean strict all-target Clippy claim.
- Existing native compiler and tape actually execute; no mocked native kernel.
  No Linux/Wasm run, CI, download, new target/worktree, or external Python execution.

A single `python3 tools/cy_check.py --measure` invocation produced 132 raw records,
including four sample-zero warmups, leaving 32 matched route/direct pairs per
backend. Source text, Host and runtime setup are outside the interval. Each arm
includes fresh parse/prepare or native compile, actual execution, status/stdout
assertions, and dropping the outcome and compiled unit. Both consume identical
source for a given pair; order alternates. The workload is a scalar print with no
Tool latency, not a comparison with Monty or a full performance campaign.

Load averages were 1.21/1.48/1.55 before and after. The process snapshot also shows
Spotlight/Finder activity, so isolated low-load conditions were not established.
Keep these values diagnostic only: tape direct/route medians 13.4585/13.5625 us,
native 150.0000/150.5415 us; paired route/direct median ratios 0.9831/1.0066.
No speedup, regression bound or broad overhead claim follows from this one run.
Do not resample or add these ratios to earlier optimization gains. Diagnostic
sampling is separate from correctness runs; `measure.json` preserves its original
source snapshot, and `checks.json` records the later final check inputs.

One focused independent read-only review approved fixed signed commit
`3e03d1de91974865787b10121208fd7949a49fbc`. The controller verified actual review
usage as gpt-6-luna/openai-codex (four calls); the separate title-generation call
is not counted as review. Prompt, output, tool trace and identity are retained in
`bench/results/cy/review-*`. The reviewer did not rerun tests. Controller separately
matched all 67 final check-input hashes, ten unchanged CV originals, and the raw
132-row summary. The original Kanban card remains for operator review.
