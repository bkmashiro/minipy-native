# DA: direct JSON i64 f-string fields

Base: approved CY `f2e905d` on main. Narrow change: consume a declared JSON i64
leaf directly in an existing print/string-Tool f-string. Native and tape share
this sink representation and execute through their real compiled/prepared paths.
No pre-sink integer lowering, generic string locals, arbitrary embedded expression,
format specifier/conversion, new dependency or fallback executor was added.
`StringSource::JsonInteger` is an additive public plan enum variant.

## Verification

Run `python3 tools/da_check.py`. `checks.json` records commands, exit codes,
toolchain, platform and hashes of all src/tests/CV originals plus the probe,
script and manifests. Reuses repository target symlink, the established fcntl
lock, one build job, Rust 1.96.0 and offline/locked dependencies.

- Full default release suite: 84 passed, 3 existing performance tests ignored.
- Targeted release tool-json and tool-json+debug-runtime: each 42 passed and
  2 existing performance tests ignored across json_fstring, string_equality,
  string_tool, string_contains, tool_json and cy_routing.
- New json_fstring suite: 7 passed in each feature combination, each covers both
  native and tape. Original CV01 source is included byte-for-byte and runs with
  ready and non-ready payloads; the latter never consumes a missing/bad count.
- Tests cover extrema, nested views, mixed string/local/bool/integer pieces,
  missing/null/type/range errors, lexical error order, exact scan boundary,
  oversized earlier strings, atomic stdout, Tool permission/call budget,
  owner lifetime across waits and last use, cancellation and detached owned text.
- Targeted Clippy passed with existing CT type_complexity allowed. Stable fmt
  and diff check passed. No full tool-json/debug suite or strict all-target
  Clippy claim. No Linux/Wasm validation was performed.

Initial targeted command (same cargo settings, tests json_fstring,
string_equality, string_tool and cy_routing) exited 101 at new test
left_to_right_scan_type_size_and_authority_priority. The test incorrectly assumed
that JSON token `"x"` costs four scan bytes. Existing validate_string charges its
three raw bytes; observed status at budget three was Ok rather than Budget.
Only this assertion was corrected to budget two failing / three succeeding.
Production budget logic was unchanged. The initial terminal output is summarized
here; final complete logs are the .txt files beside this document.

## One bounded diagnostic

Run `python3 tools/da_check.py --measure`. `raw.csv` has 132 records, four sample-zero
warmups, leaving 32 direct/temporary pairs per backend. `summary.json` is a pure
reduction; `measure.json` preserves command, exact binary/source hashes, toolchain,
load and process snapshot for that invocation. No resampling or pooling.

Both arms use new source per sample. Direct `print(f"count={d['count']}")` is
compared with the existing `n=d['count']; print(f"count={n}")` path. Text creation,
Host/declaration/runtime setup are outside the timer. Fresh parse+prepare/native
compile, actual execution, status/stdout assertions, and unit/outcome destruction
are inside. In-memory provider has no intentional Tool latency. This comparison
measures two source forms on this implementation, not a pre/post optimization.

| Backend | Direct median | Temporary median | Paired direct/temporary median |
| --- | --- | --- | --- |
| tape | 20.0625 us | 23.1245 us | 0.8662 |
| native | 159.9795 us | 180.3130 us | 0.8933 |

Load before/after: 1.482/1.413/1.358. Snapshot alone does not establish isolated
low-load conditions; there is no during-run CPU trace. These are synthetic
single-round diagnostics, not evidence of general net gain, a regression bound,
Monty competitiveness or production coverage. No additional timing campaign.

Current CV matrix: 01/02/03/06/07/08/10 run as complete original synthetic scripts.
04 arrays/iteration/break, 05 get/default, and 09 startswith remain unsupported.
The count is not a workload coverage percentage. CY documentation keeps its
historical matrix and adds an explicit DA current-state note.

One focused read-only Luna review will target the signed integrated commit;
its exact commit, output and actual-model verification are recorded separately.
