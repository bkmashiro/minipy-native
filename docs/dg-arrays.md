# DG: bounded declared object-array iteration

## Result and scope

From reviewed DF `4fc5564`, `tool-json` now prepares and executes the unchanged
`fixtures/cv/04_first_matching_result.py` on the shared tape and real Cranelift
native backend. This closes the final gap in the ten synthetic CV originals.
Ten out of ten is a property of that small synthetic set, not a production
coverage percentage or support for general Python collections.

The Host opts into one object-array result per Tool:

```rust
ResultSchema::JsonObjectArray(Box::new(ObjectArraySchema {
    path: vec!["results".into()],
    i64_fields: vec!["score".into(), "id".into()],
}))
```

`path` is a nonempty literal object-key path of at most 31 segments. Keys and
field names are at most 256 UTF-8 bytes. `i64_fields` is nonempty, unique, and
contains fewer than 128 fields. The existing total bound of 128 *used* path slots
also applies, including parent segments. Validation happens at declaration
import/preparation. Array elements need not contain unvisited fields.

A source `for item in data["results"]` binds an iteration-only object view.
Existing unconditional root/subview aliases, including an alias of the array,
can select the same declared path. `item["score"]` and `item["id"]` resolve to
static i64 field slots. General existing scalar arithmetic, assignment, conditions,
short circuiting and scalar print work in the body. `break`, including nested
conditional breaks, exits this array loop and skips its remaining statements.
Accumulators initialized before the loop retain their values on empty input.
Loop-only initialized scalars are not definitely initialized after a possible
empty loop. No element view escapes the iteration.

Not supported: dynamic/integer indices, `len(array)`, array construction, general
list/dict locals, nested loops/arrays, dynamic fields, element aliases/objects,
string element fields, `continue`, `for-else`, Tool calls inside loops, array
rebinding or generic iteration. Quote loops retain their existing contract and
do not gain break. Unsupported source is rejected before executing any effect.
The Host routing example still never replays runtime errors.

## Representation and work

The new `Stmt::JsonFor` and `Stmt::JsonBreak` share the existing frontend.
Row reads reuse `Expr::JsonI64`. Native lowering emits source-dependent CFG for
the conditions, checked operations and break, with the existing continuation
mechanism only at real chunk/Host boundaries. The AOT array helper performs
cursor/value parsing and fuel accounting, not the user's predicate or reduction.
The tape has ArrayStart/ArrayStep/ArrayFinish plus ordinary arithmetic and Jump.
There is no native fallback to an interpreter or Host business kernel.

A JSON task has an optional boxed 56-byte ArrayCursor. It is allocated only on
entering a loop, after the metadata budget check. It retains root/slot, the
selected array range, a byte position, visited count and chunk count. It never
stores a self-referential parser, DOM, per-element object or full range table.
Existing safe sonic-rs lookup obtains a checked immutable array range. Each
visited element uses safe serde_json StreamDeserializer<IgnoredAny>, taking its
byte_offset as a checked borrowed owner range. Only comma/closing-bracket framing
is handled locally. The parser's per-value transient stack/scratch is not retained
in the task and is not advertised as allocation-free.

The array path's cached range temporarily identifies the current object; its
one-level child ranges/i64 caches are cleared when advancing. Finish/break restores
the original array range and removes the cursor. Owner replacement and last-use
release also clear it. No row bytes are copied, and rows do not become new roots.
Sequential loops start fresh; prepared units own no per-execution cursor.

## Validation and observable order

The array itself is required and must have array type. Every reached element is
structurally parsed and explicitly checked for UTF-8 before its body, and must be
an object. Required reached fields use the existing safe first-match lookup and
signed-i64 decoder. Escaped object keys work; duplicate keys retain MiniPy's
existing first-match policy (CPython's dict construction has a different policy).
Null, bool, float, string and out-of-range integers are not i64 values. Missing
reached fields and invalid reached containers are task Json errors.

Unvisited fields do not get i64 conversion. Structural IgnoredAny permits an
unvisited surrogate escape without converting it to a Rust String, consistent
with the earlier bounded missing-object parser proof. Raw invalid UTF-8 in a
visited object's otherwise unused value is rejected. Existing required/present
paths outside this operation and outer bytes after the selected container retain
their lazy behavior; this is not whole-document validation.

Safe container lookup can scan the complete selected array before the first
body. A malformed syntactic tail may therefore fail even if the first body would
break. Break avoids later body/field conversion, not necessarily scanning or
syntax checking the tail. No claim of constant-cost first-match lookup follows.
Tests keep a reached error, a skipped wrong-type element and malformed-tail cases
separate. Earlier stdout survives errors; failed atomic lines remain uncommitted.
Checked arithmetic and short-circuit error ordering remain source ordered. A
failure prevents subsequent Tool dispatch. No fallback/replay is introduced.

## Resource and cancellation policy

Engineering bounds specific to this slice:

- The selected array range is at most 65,536 bytes, checked after safe lookup.
  Its entire JSON owner is also bounded by the existing aggregate buffer cap.
- At most 256 elements may be visited per loop. Element 257 fails Budget before
  its body. An earlier break need not reject a longer syntactically valid array.
- Visited elements consume one unit from the existing cumulative `Limits.rows`,
  shared with Quote scans and later array loops. Empty arrays consume zero.
  Unlike Quote's unchanged chunk reservation, array fuel charges each visited
  element, so break does not precharge unvisited elements.
- Chunk size remains 1..4096. Once a chunk has been processed, another candidate
  element causes the existing yield boundary. Root masks keep the cursor's owner
  live across it. Cancellation is checked by the existing dispatcher after yield
  and at entry/Tool waits. No new scheduler or service-wide quota is implied.
- Cursor bytes count against the existing aggregate buffer cap along with JSON
  owners, decoded strings and any live Quote buffers. Exact 55/56-byte headroom
  and mixed Quote/JSON boundaries are tested. Transient parser work is bounded by
  the array size and parser recursion bound, not charged to string scan tokens.

## Costs and API migration

Actual macOS arm64 / rustc 1.96.0 layout:

| Item | Before | After |
|---|---:|---:|
| ResultSchema | 32 | 32 |
| ToolDeclaration | 88 | 88 |
| Plan | 112 | 112 |
| tape Op | 48 | 48 |
| tape State / unit | 312 / 112 | 312 / 112 |
| native Frame | 320 | 320 |
| JSON task State | 128 | 136 |
| no-array four-op prepared tape bytes | 304 | 304 |

The public schema/declaration before shapes are also compiler-laid-out in a
regression test. An initial inline two-Vec schema measured 48/104 bytes and was
replaced with the boxed payload; only array declarations allocate that payload.
No-array tasks do not allocate a cursor or additional path/string cache. Every
JSON task, including old no-array JSON tasks, still pays the extra 8 inline bytes.
The Plan flag uses existing padding. Array bodies add tape operations or native
code and use one cursor allocation per entered loop. The no-array lowering's empty
break-edge Vec does not allocate. Source inspection finds no additional no-array
per-task allocation site; no allocator-count or RSS campaign was performed.

Concrete public execution futures with the test Host measure native 664 and tape
928 bytes, matching earlier documented sizes; no frame fields or await sites were
added. These are not generic bounds for arbitrary Providers or process-memory
measurements. Output remains 72 bytes. Raw layout diagnostics are in the results.

`ResultSchema` gains an exhaustive-match case and `ObjectArraySchema` is new.
`Plan` gains public `array_paths: bool`; full struct-literal callers need that
field (false for old plans), or `..Plan::default()`. `Stmt` gains JsonFor/JsonBreak
under `tool-json`. Provider, Reply, ArgumentSchema, scalar Type, public execute
entry points, Cargo dependencies and string/get representations are unchanged.
The Plan is a sealed compiler representation; Hosts should use declarations and
the existing preparation APIs rather than manufacture executable IR.

## Evidence and reproduction

`bench/results/dg/` contains command-bearing raw logs, aggregate validation,
CPython oracle results, source hashes and toolchain identity. Existing mounted
external target `/Volumes/M.2/caches/minipy-native-target` was reused, under
`/Users/yuzhe/.hermes/cache/scratch/minipy-perf-build.lock` with one build job,
`RUSTUP_AUTO_INSTALL=0`, Rust 1.96.0, `--offline --locked --release`. No new target
or worktree, downloads, CI, VM/Linux/Wasm run or performance campaign.

Main commands (wrap Cargo in the shared fcntl lock before reproducing):

```text
python3 tools/dg_array_oracle.py
cargo +stable fmt --all -- --check
cargo +1.96.0 test --offline --locked --release
cargo +1.96.0 test --offline --locked --release --features tool-json
cargo +1.96.0 test --offline --locked --release --features tool-json --test json_array -- --nocapture
cargo +1.96.0 test --offline --locked --release --features tool-json,debug-runtime --test json_array --test json_get --test co_count --test debug_runtime
cargo +1.96.0 test --offline --locked --release --features tool-json --lib dg_array_source_cfg -- --nocapture
cargo +1.96.0 clippy --offline --locked --release --lib --test json_array --test string_equality --features tool-json,debug-runtime -- -D warnings -A clippy::type_complexity
git diff --check
```

The 12 integration tests run both backends and cover CV04, empty/single/multi
first-match/no-match, static aliases, duplicate/escaped keys, general bodies,
cache reset, unreachable code after break, reached/skipped invalid fields,
malformed tails/UTF-8, arithmetic/permission-free ordering, exact row/byte/metadata
budgets, stdout cap, real Host waits, cursor owner across waits/chunks, last-use
release, wait/chunk cancellation, prepared-unit reuse, mixed Quote fuel and
buffer caps, and whole-source rejection. Internal tests additionally check
replacement/release and real source-dependent native CLIF.

Initial failures were the expected preimplementation missing schema variant,
the old 128-byte state assertion detecting real growth, one missing test import
and a test Clippy initializer. No behavior assertion or benchmark gate was
relaxed. An attempted strict all-target Clippy also hit existing ct_schema
useless_format/manual_strip findings; its failure log is retained. Focused
production/new-test Clippy passes with only the existing CT type_complexity
allowance. This is not an all-target lint-clean claim.

Final counts and independent fixed-commit review are recorded in experiments.md
and the Kanban handoff. No latency, speedup, Monty comparison or production
coverage claim is made. Further arrays/strings/iterator features require a new
controller decision, not automatic continuation of this slice.
