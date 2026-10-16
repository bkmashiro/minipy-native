# DF: shared declared-string get/default

## Scope and semantics

With `tool-json`, a bound JSON root, static object path or existing object alias
can use `.get(key_literal, default_literal)` at existing string sinks. The full
leaf path must be declared `JsonLeaf::String`. This is one shared string source
for print, single-string Tool arguments (including the outer Tool keyword), `+`
and limited f-strings. CV05 is unchanged and now executes on native and tape.
Nine of the ten synthetic CV originals now run; only CV04's arrays/iteration
remain unsupported. This is not a production coverage estimate.

Only the final missing object key selects the literal. Required parents still
fail on missing/null/non-object values. Present empty strings stay empty;
present null, numbers, bools, arrays and objects fail with `TaskError::Json`.
Duplicate keys retain first-match behavior, including escaped key spellings.
Direct leaf/object aliases retain eager validation at assignment.

Key/default must be positional string literals. Both are checked during whole
source preparation, even in an unexecuted branch or when the field is present.
The existing source-surrogate rejection and 4096 UTF-8-byte literal limit apply.
A get is one of at most 32 sink pieces; source remains capped at 16 KiB. Unknown
full paths are Schema, declared non-string leaves are Type, unsupported syntax
is Unsupported, and literal/piece/source limits are Resource. General string
locals, get predicates, omitted/keyword/expression defaults, nested get, dynamic
keys, arrays and arbitrary methods are not admitted. Preparation precedes effects;
execution errors never trigger routing or replay.

## Missing-only proof and costs

`State::get_present` first resolves required ancestors using existing `locate`.
The receiver must be an object. A safe sonic single-key lookup returns Found,
non-NotFound error, or a candidate absence. Found imports the checked borrowed
range and follows existing string validation/decoding, without full-object
validation. Required `[]` keeps its old behavior and diagnostics.

Candidate absence alone is insufficient: sonic NotFound accepts `{garbage}` and
can skip invalid UTF-8. As proved by [DE](de-missing-parser.md), a root receiver
is selected with safe empty-path get and checked offset/end/within bounds. A
parent receiver already has an exact checked range. Only this selected object
is validated with explicit `str::from_utf8` and `serde_json::IgnoredAny` structural
parsing. No retained DOM or leaf conversion is introduced. Invalid syntax or
UTF-8 within it fails. Outside suffixes are not newly validated. Unvisited
surrogate escapes remain structural syntax, not requested String-domain decoding.
Present and required access retain the existing lazy contract, including the
possibility of accepting malformed unvisited tails.

Absence has no token and consumes no fictional string/decode scan bytes. The
extra lookup, root selection and UTF-8/structural validation can scan the object
multiple times, and repeated missing reads repeat this work. Existing owner and
source/piece bounds limit its input; `string_scan_bytes` does not meter this
lookup/proof work. Parser transient scratch is not fully charged to buffer_bytes.
This synchronous helper has no new per-byte cancellation or CPU-slice guarantee.
No latency, allocation-free, single-pass or Monty advantage is claimed.

Present tokens charge existing raw bytes once per consumption and escaped first
decode once more. Existing decoded capacity, 64 KiB raw token/join bounds,
aggregate buffers, stdout/newline, permission and Tool count checks remain.
Print uses a local u32 selector in its two-phase prepare/commit path, with no
second lookup/charge at commit. Failed lines remain atomic. Tool arguments are
fully evaluated before authorization and are not submitted on evaluation failure.
The shared native/tape direct-argument helper preserves small-leaf detachment,
including low-headroom fallback. Joined arguments keep their existing policy.
Roots remain live across real waits and release at last use; submitted operations
own plain Bytes, decoded Arc or default Arc independently after cancellation.

## Public API migration and retained size

New public variant:
`StringSource::JsonGetLiteral { slot: usize, root: usize, default: Arc<str> }`.
Downstream exhaustive matches must add a branch. `slot` names the complete path;
`root` participates in liveness; the default belongs to this expression, never to
PathSlot. Same-path defaults and required reads therefore cannot share fallback
state. Provider, Reply, StringValue, schema, IR operations and entry points are
unchanged. No dependencies, Missing cache, Plan flag or task field were added.

Measured on aarch64 macOS with rustc 1.96.0 (ac68faa20), using exact pre-get enum
and BoundCall shapes from 5a47c2c in a unit test:

| Inline layout | Before | After |
| --- | ---: | ---: |
| StringSource / Option<StringSource> | 24 / 24 | 40 / 40 |
| BoundCall | 40 | 56 |
| Plan | 112 | 112 |
| JSON State | 128 | 128 |
| StringCache | 24 | 24 |

This costs 16 extra bytes per retained call or Joined/print source capacity slot,
including plans that do not use get. `owned_bytes` uses actual element sizes and
includes each default's UTF-8 bytes, following the existing payload accounting
convention (not allocator-header/RSS accounting). A get default has an Arc payload
allocated during preparation. Source inspection confirms no additional per-task
allocation site or missing metadata on no-get paths; this is not a measured
before/after allocator-count or latency comparison. Scalar-only execution still
creates no JSON state. The new print selector is stack-local, not task storage.

## Verification

Baseline: signed 5a47c2c8aad8b3dfa655509ae88b519b5b7ea0f5, clean main.
Platform: macOS 26.4.1 aarch64, rustc 1.96.0 / LLVM 22.1.2. Existing target symlink
resolves to `/Volumes/M.2/caches/minipy-native-target`. No new target/worktree,
download, Linux/Wasm execution, benchmark, CI or website change.

All Cargo operations hold a nonblocking exclusive fcntl lock on
`/Users/yuzhe/.hermes/cache/scratch/minipy-perf-build.lock`, with
`CARGO_TARGET_DIR=/Users/yuzhe/projects/minipy-native/target`,
`CARGO_BUILD_JOBS=1`, `RUSTUP_AUTO_INSTALL=0`.

- Before implementation: all 7 initial json_get tests failed at admission.
- Default full release: 84 passed, 3 existing diagnostics ignored.
- tool-json full release: 172 passed, 13 existing diagnostics ignored.
- tool-json + debug-runtime json_get/equality/string_tool/routing: 28 passed,
  2 existing diagnostics ignored.
- New json_get suite: 8 tests, each runtime test executes both real backends.
  Unit tests cover found/decode invalidation, absent non-caching and layout/cost.
- An initial full feature run exposed the historical CV05-rejected assertion;
  it was moved to successful execution, while CV04 rejection remains checked.
- Initial layout expectations were corrected to actual measured sizes, rather
  than retaining guessed numbers. No acceptance limit was relaxed.

Commands (after acquiring the lock and setting the environment above):

    cargo +1.96.0 test --offline --locked --release
    cargo +1.96.0 test --offline --locked --release --features tool-json
    cargo +1.96.0 test --offline --locked --release --features tool-json,debug-runtime --test json_get --test string_equality --test string_tool --test cy_routing
    cargo +1.96.0 test --offline --locked --release --features tool-json --lib get_ -- --nocapture
    cargo +1.96.0 clippy --offline --locked --release --features tool-json --lib --test json_get --test string_equality -- -D warnings -A clippy::type_complexity
    cargo +stable fmt --all -- --check

Clippy retains the existing type_complexity exception; this is not a strict
all-target Clippy claim. Linux validation remains pending. One fixed integrated
snapshot receives focused read-only Luna review; same-card human review remains
the final gate.
