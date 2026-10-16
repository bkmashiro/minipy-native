# DR: borrowed lines (experimental branch only)

This candidate is not merged into main and is not an API release. It admits
`for line in declared_string.splitlines():` in the shared frontend, then lowers
the ordinary user body to the actual tape or Cranelift CFG. Helpers only provide
cursor movement and string predicates. There is no Host reduction or backend fallback.

## Contract

- Receiver: a declared JSON string leaf or existing single-binding leaf alias,
  with dot/subscript selectors. Only a direct for-iterable call with no arguments
  or keywords. No list result, keepends, dynamic receiver or general string local.
- Boundaries match CPython `str.splitlines(keepends=False)`: LF, CR, CRLF, VT, FF,
  U+001C/1D/1E, NEL, LS and PS. Empty input has no lines; a trailing separator adds
  no final empty line. Consecutive separators retain internal empty lines.
- At loop entry validate the entire selected UTF-8 string, using existing raw-token
  limits (65536 bytes including JSON quotes/escapes). Escaped strings decode once
  into the existing cache; plain strings borrow the original owner. Invalid UTF-8,
  surrogate escape, missing/null/wrong-type selected values fail before any body.
  Unvisited JSON siblings/suffixes keep the existing lazy contract.
- `line` is a temporary, iteration-only view, with literal `in line` and
  `line.startswith(literal)` (4096 UTF-8 byte literal limit). Existing bool/not,
  short-circuit, checked scalar operations, if and break work. Print consumes the
  current line through the existing synchronous, atomic borrowed-string sink.
  Existing literal joins/f-string print pieces can include the view, without
  creating a string local. No equality, line rebinding/alias, general methods,
  Tool argument escape, nested loops, continue or for-else. Loop-local line use
  after the loop is rejected for the entire source, even if nonempty. Python's
  persistent final loop variable is deliberately outside this subset. New scalars
  defined only in the body remain not definitely initialized after an empty loop.
- Tools remain forbidden inside loops and follow normal authorization outside.
  Owners survive backedges/yields and future uses, then release at existing last-use
  boundaries. Finish drops only the cursor, not a later-used decoded string cache.
- Every delivered line charges one existing cumulative rows fuel unit, including
  empty lines. Empty input charges zero. No separate 256-line limit. Scanning
  charges actual forward UTF-8 bytes including separators/CRLF; predicates charge
  line bytes plus literal bytes; print charges current line bytes. Escape decoding
  retains its existing raw-token charge. Plain entry validation/parser work has
  the existing bounded behavior, not an exact scan-meter accounting claim.
- Existing chunk size bounds and scheduler cancellation checks are unchanged. A
  chunk yields only with more candidate text. A long single line can run up to the
  existing token bound synchronously. There is no wall-clock preemption claim.
  Break scans no following line; short-circuit skips unselected predicates.
  A failed scan is terminal and commits no row/body. Failed partial scanning is
  not written back to the meter, which cannot be retried by user code.
- Print validates/measures parts before permission/budget checks and commits the
  complete output plus newline atomically. Prior stdout survives any later error.
  Only the final stdout buffer receives line bytes, no intermediate per-line copy.

## Representation and compatibility

Public exhaustive matches must add `Stmt::LineFor`, `Stmt::LineBreak`,
`Expr::LinePredicate` and print-only `StringSource::Line`. No existing public
function signature, Type, Plan field, global budget, source ordering, effect
contract or array 256-element semantics changes. These new enum alternatives
remain experimental and require an explicit integration decision.

A shared optional tagged cursor holds either `Box<ArrayCursor>` or
`Box<LineCursor>`. This intentionally chooses an 8-byte inline tag over the DQ
modeled `Box<union>`: that union would grow the old array allocation from 56 to
64 bytes and change its exact aggregate-budget boundary. Existing array metadata
still costs exactly 56 bytes; the new line cursor costs 48. No-feature JSON
State grows from 136 to 144 bytes with no additional allocation call. Scalar
programs do not create a JSON State. No self-referential borrow, retained line
list, range table or per-row owner is introduced.

## Evidence and reproduction

All builds use the mounted existing target, one job and the shared fcntl lock:

    python3 bench/dr_checks.py regression.json test --offline --locked --release --features tool-json,debug-runtime --lib --test line_scan --test json_array --test scripts --test debug_runtime --test string_tool --test string_contains --test json_get --test string_equality -- --test-threads=1
    python3 bench/dr_checks.py default.json test --offline --locked --release --test scripts --test interpreter -- --test-threads=1
    python3 bench/dr_checks.py costs.json test --offline --locked --release --features tool-json,debug-runtime --test dr_cost -- --ignored --nocapture --test-threads=1

The wrapper pins Rust 1.96, sets target/jobs/no-auto-install and records raw output
and source/toolchain identities under `bench/results/dr/`. Rustfmt 1.8 from the
existing stable toolchain is used: the pinned toolchain has no rustfmt component;
no component was downloaded. Synthetic oracle data uses CPython splitlines on
181 inputs, both backends and forced-escaped variants (724 executions). This is
mechanism coverage, not a workload distribution or Agent success-rate result.
Debug single-instruction tape execution tests line/array/line sequential cursors,
break, yield and predicate destination reporting; the public legacy debugger has
no declared-JSON Tool setup API, and no such API was added.

Private DP08 count and filter hand-written complete scripts run through the real
DI replay wrapper in both timing and ownership modes (eight case/backend/mode
rows, all correct). Frozen DP inputs/oracles/original answers are unchanged. Raw
source/payload/results remain in the authorized private corpus, not Git. No new
model answers or Agent-success-rate increment. Only bounded synthetic fresh
prepare/execute/drop observations, no Monty baseline or stable speedup claim.
Linux, Wasm, total native metadata and RSS/PSS are unmeasured.
