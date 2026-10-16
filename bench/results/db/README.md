# DB: statically bound JSON string prefix

Base: DA reviewed main `d46ff94d3b21fe2744dfbfab3205d5a8500197ac`.
The opt-in `tool-json` frontend accepts only a declared string leaf or its
existing alias, one literal positional prefix, and bool result. No general
method dispatch, string locals, dynamic/tuple prefixes, optional start/end,
endswith/find/slicing, special Host Tool, dependency or fallback is introduced.

## Contract and implementation

- Prefix literals: at most 4096 UTF-8 bytes (engineering bound inherited from
  equality); source surrogate escapes reject using the shared literal checker.
  Exact decoded Unicode prefix, no normalization or case folding. Empty prefix
  is true only after the usual runtime locate/charge/validation succeeds.
- Each evaluation: locate token, charge its raw length plus prefix UTF-8 length,
  then validate/decode. First escaped decode separately charges raw token bytes.
  Repeat cache hits still charge each predicate. Existing 64-KiB raw token and
  cumulative scan/retained budgets apply. Missing path wins before charge;
  insufficient charge after locate wins before type/decode error.
- IR has one additive public `Expr::StartsWith` variant; exhaustive downstream
  IR matches must handle it. Tape has one sealed operation. Native uses the
  existing Rust string helper, keeping its ABI shape: -1 validate, nonnegative
  contains, -2-2*index equality, -3-2*index prefix. This selector is generated
  only by trusted lowering, with literal count bounded by the existing source/
  node limits. Native and tape call the same State predicate implementation.
- Root masks protect owners across waits and release at last use. Short-circuit
  skips unexecuted direct accesses; alias assignment retains its eager validation.
  Cancellation never replays or rolls back a submitted effect.

## Actual checks

`python3 tools/db_check.py` uses the existing target symlink to
`/Volumes/M.2/caches/minipy-native-target`, shared fcntl build lock, one build job,
Rust 1.96.0 offline/locked and existing stable fmt. `checks.json` records all 67
source/test/fixture/manifest/tool inputs, toolchain identity, commands and logs.
Controller verified all 67 against disk, and all ten original CV fixtures match
base byte-for-byte.

- Full ordinary default release: 84 passed, 3 existing performance tests ignored.
- Six targeted `tool-json` suites: 46 passed, 2 existing performance tests ignored.
- Same suites `tool-json,debug-runtime`: 46 passed, 2 ignored.
- Feature library: 30 passed, 3 existing performance tests ignored.
- Focused release Clippy: passed, allowing only the existing CT type_complexity
  lint. Not a strict all-target Clippy claim. Stable fmt and diff check passed.
- Four new prefix tests each cover both actual backends where execution applies:
  Unicode/NUL/escaped and plain values, empty and too-long prefix, case and
  normalization distinction, direct/alias bool locals, mixed selector indices,
  missing field/wrong type/lone surrogate/invalid escape/UTF-8, precise charging
  and repeated cache use, retained/token/stdout limits, source admission bounds
  and unsupported method forms. The existing lifecycle matrix now tests both
  equality and prefix on both backends: resume, cancellation, last-use release,
  budget across wait and detached plain/decoded Tool argument ownership.
- CV09 uses unchanged source and checks exact stdout plus Tool argument/order
  for draft and final branches. Existing CV01/02/03/06/07/08/10 run in their
  regression suites. Eight synthetic originals run; 04 arrays/iteration/break
  and 05 get/default remain unsupported. This is not production coverage.
- Default full tests, old contains/equality, routing, JSON/f-string/Tool tests
  serve as controls not using the new operation. No timing control establishes
  absence of old-path performance regression.

The first focused test compile hit a test-only `limits.limits.stdout_bytes`
field typo (E0609); corrected to `ScriptLimits.stdout_bytes` before the scripted
checks. Initial scripted checks passed tests but Clippy rejected a test-only
field-reassign-with-default. Preserve `initial-checks.json` and
`initial-clippy.txt`; struct initialization corrected it. Production budget or
validation rules were not changed in response to either failure.

No CI, Linux/Wasm execution, new target/worktree, downloads or system changes.

## One bounded diagnostic

`python3 tools/db_check.py --measure` builds/runs `examples/db_probe.rs` once.
`raw.csv` has 132 rows, including four sample-zero warmups. Each backend has
32 direct/alias pairs in alternating order with unique source text per sample.
Both arms actually fetch a JSON reply and print the prefix result. Runtime,
Host, declarations and source-text creation are outside timing. Fresh parse/
prepare or native compile/install, execution, assertions and compiled-unit/
outcome cleanup are inside. No source cache or fixture-specific kernel.

Recompute with CSV and Python statistics.median, discarding sample zero and
pairing each backend/case by sample. `summary.json`:

| Backend | direct median ns | alias median ns | paired direct/alias median |
| --- | ---: | ---: | ---: |
| tape | 20916.5 | 25020.5 | 0.8483416576456844 |
| native | 199563.0 | 226999.5 | 0.8757167940537911 |

This is a source-form diagnostic of the new capability, not pre/post evidence.
The alias source has another binding and eager string validation/charge; it is
not an equivalent-budget substitute. Load before/after is 1.94/1.78/1.54 and the
process snapshot includes Finder and Spotlight activity, so isolated low load
is not established. Do not infer optimization speedup, no-regression bounds,
Monty competitiveness or production distribution coverage. No replacement
sampling or full campaign. `measure.json` preserves binary hash and original
source snapshot separately from correctness checks.

One fixed-commit Luna read-only review follows this integrated snapshot. Final
review verdict and independently verified actual model identity will be recorded
alongside the prompt/output/trace. Same-card operator review remains the release
gate, not a second implementation or broad optimization phase.
