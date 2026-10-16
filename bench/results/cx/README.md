# CX bounded string equality evidence

Base: signed main `a2b5b2f2550de1e809948ac52b3fd071fb2fbca4`.
The opt-in leaf/literal equality implementation adds no dependency or general
string value. `tests/string_equality.rs` runs real tape and Cranelift source paths.
All ten `fixtures/cv/*.py` are byte-identical to the reviewed CV report cases.
Six original scripts now execute (02,03,06,07,08,10), versus four statically
predicted before CX. 07/08 are the only newly unlocked originals. 01 still fails
on the direct JSON-i64 f-string at line 3, 04 lacks iterable JSON representation,
05 lacks get/default, and 09 lacks startswith. These are synthetic examples, not
production coverage. Test Host performs only declared in-memory data/effects.

## Correctness and identity

`check_identity.json` and default/tool_json/debug_json.txt are the earlier full
non-release suites. Reconciliation verified all 36 source hashes unchanged:
80/140/148 passed respectively; 3/13/13 ignored. This is reused execution evidence,
not a claim that this executor reran those full suites.

`python3 tools/cx_finalize.py` performs byte identity checks, raw summary reduction,
and locked single-job offline checks using the existing external target symlink.
`reconciliation.json` records exact final source hashes, commands and exit codes.
Final independently run release tests: equality, contains, string_tool and
local_read_host (18 passed, two existing performance tests ignored), plus six
equality tests with debug-runtime. Targeted
library/new-test/example Clippy passes with existing CT type_complexity allowed;
there is no claim of clean strict all-target Clippy. Stable fmt and diff check pass.
Failures are retained: typo `tool_strings` (no such target), CT frontend type
complexity, CT test useless_format/manual_strip, and missing 1.96 rustfmt. We used
the already installed stable formatter instead of downloading a component or
editing unrelated CT code. No Linux/Wasm rerun, new target/worktree, CI or download.

Tests cover both literal sides and !=/==, aliases and short-circuit, Unicode,
combining characters without normalization, NUL, empty/escaped strings, bad
UTF-8/missing/null/type/surrogates, exact repeated/decode budgets, 64-KiB raw token
bound, retained bytes, stdout/error/effect order, root across waits, last-use
release, cancellation and detached string argument ownership. RustPython 0.4
silently replaces source surrogate escapes; equality checks raw lexer token
spelling only when U+FFFD occurs and rejects surrogate escapes. Genuine U+FFFD,
raw strings, escaped backslashes, comments and implicit concatenation still work.

## Timing: two separate diagnostic batches, no performance conclusion

Two old workers overlapped on this shared directory. Both have exited. Preserve:

- `raw.csv`, `summary.json`, `measure_identity.json`, `build.txt`: 3 processes,
  594 rows including 18 sample-zero warmups, 576 remaining observations.
- `run-0.csv` through `run-2.csv`, `identity.json`, `load-*.txt`: separate process
  batch, not merged into the first. Its identity records a different probe hash.

Both batches are downgraded to diagnostic-only because exclusive low-load
conditions were not established. Load averages/top snapshots do not prove
absence of concurrent work. Existing summaries are arithmetic descriptions only:
tape eq/contains/scalar median 14.1875/14.5415/12.7705 us; native
127.8545/128.3335/116.1455 us in raw.csv. They establish no speedup, regression
bound, or low-load acceptance. Do not add them to previous optimization gains.
No third sample campaign was run.

The probe includes source preparation, actual execution and unit/outcome drop.
Host/runtime/declaration setup and generating each new source text are outside
the interval. Equality and contains are different operators on one controlled
string, not a before/after comparison or a realistic workload distribution.
`tools/cx_check.py --measure` documents the earlier invocation; do not rerun it
as part of reconciliation. Source checks/testing and timing are separate.

One fixed signed commit review examined `e647836`. `review-output.txt` reports
no blocking findings and explicitly says the reviewer did not rerun tests.
`review-identity.json` verifies eight actual gpt-6-luna/openai-codex review calls
from the session usage table; the separate title-generation call is not review.
The original CX card records the final remote SHA and review handoff.
