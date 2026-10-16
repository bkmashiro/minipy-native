# CT: preparation-only borrowing of matched Tool schemas

Decision: retain the small borrowing change. It deletes declaration/schema copies without changing runtime state, public APIs or ownership. No general latency improvement is claimed. Do not expand this matrix or start another frontend optimization from these results.

## Scope and identities

Baseline is CS main `96a59548eed8f0f128437c24e72be21c1fed281e`. `Compiler::compile_tool` really cloned the matched `ToolDeclaration`, including name, argument names and all result paths, for every call. The candidate takes a declaration reference and stores a private `SchemaLeaves(&ResultSchema)` per JSON root during preparation. A two-slice iterator presents both numeric and typed schemas without converting either. Only bound paths enter the existing owned Plan. The Compiler and borrowed schemas are gone before prepare returns. Neither a task-owned environment nor a new public API is introduced.

Formal A/B binaries were built with Rust 1.96.0, offline/locked, release, `tool-json`, no PGO and the existing Cargo release profile. The exact source hashes, Cargo/lock hashes, toolchain host, binary SHA256 and candidate diff are in `evidence/ct-schema/ct-{a,b}-identity.json`. The adapter `tests/ct_schema.rs` is identical in both binaries. A has baseline production sources; B differs only in `src/frontend.rs`. Both used the existing external target. B's frontend was restored byte-for-byte after the temporary allocation-diagnostic baseline build. Later diagnostic-timer changes are a separate commit and were not in these formal binaries.

## Prespecified matrix and interval

The Kanban precommitment fixes 2/64 schema leaves × 1/4 calls × 1/2 used leaves, plus scalar, numeric Tool, string argument and invalid path controls, for both tape/native. There is one ABBA and one independent A/A sequence per cell. Every process creates one registered environment, Host and current-thread runtime, executes 64 warmup requests, then 48 measured requests. Each request gets a distinct source string; source construction models arrival and is outside timing. Parsing, binding/lowering, execution, fresh producer JSON allocation, code/output/result cleanup and fixed correctness assertions are inside. There is no prepared-unit cache or precompiled business result. For an admission-error cell the interval ends after the error is released; no effects execute. Environment registration is outside the request interval. Sample logging is outside timing. The independent allocator binary is never used for formal timing.

The collector holds `~/.hermes/cache/scratch/minipy-perf-build.lock`, requires the existing mounted external target, and records `top` output. Preflight requires the last two samples ≥90% global idle and no unrelated process ≥50% of one core. Two consecutive busy live samples stop the campaign (85% idle live threshold). These thresholds are an engineering guard, not proof of no per-request preemption. This short run recorded three live blocks, none flagged busy. The sparse CPU evidence cannot explain frequency/thermal/process drift.

## Results and limits

192 processes, 9216 measured requests, all result assertions passed. Raw stdout/rows, binary identities and CPU observations are committed under `evidence/ct-schema/`. `summary.json` uses per-process medians, then medians of the two process medians per arm. It does not pool all requests as independent processes.

Wide-schema tape B/A is 0.639–0.782 across the four cells; narrow tape excluding the unstable first cell is 0.924–0.968. Native wide B/A is 0.956–0.990, with no consistent broad native win. Scalar/numeric/string native controls are 1.018/1.005/1.039. Independent A/A ranges 0.981–1.056 across all cells. Two processes per arm and A/A do not establish an error bound or a causal layout diagnosis.

The first narrow one-call/one-leaf tape cell is explicitly unstable: A process medians 5708.5/8875 ns, B 21208.5/11583 ns, combined B/A 2.249. Its later A/A ratio is 0.996. This is an observed regression/drift in the formal record, not discarded data and not explained by the low-frequency CPU monitor. It prevents claiming a small-input no-regression result. No replacement or extended campaign was run. Retention rests on the simple, proven removal of copies, with supporting wide-schema synthetic signal; it is not a production workload claim.

Independent allocation diagnosis, same preparation request using one leaf:

| Schema | Calls | A allocations, 2 / 64 leaves | B allocations, 2 / 64 leaves |
|---|---:|---:|---:|
| numeric JSON | 1 | 57 / 181 | 50 / 50 |
| numeric JSON | 4 | 183 / 679 | 155 / 155 |
| typed JSON | 1 | 56 / 180 | 50 / 50 |
| typed JSON | 4 | 179 / 675 | 155 / 155 |

These are allocation calls while preparing on a warmed parser thread, not bytes, retained-memory measurements or runtime latency. Deallocation after prepare is not counted. For this independent diagnostic A production source was temporarily restored exactly to CS (`git diff --exit-code -- src/frontend.rs`), diagnosed once, then B was restored and checked against the formal B hash. The separate test asserts unused width does not add preparation allocations for either result-schema variant.

Correctness: both schema variants and four repeated calls use an owned Plan after destroying Environment, on real tape/native execution. Fourteen baseline structured admission errors match kind, full reason and exact line/column, including wrong type vs unknown argument, duplicate vs bad value, string keyword precedence, undeclared path and intermediate view. Existing environment/drop/permission tests, source-order, overflow, budgets, cancellation and actual fused CO tests are retained. Default debug suite: 79 passed; tool-json debug and release without debug-runtime: 133 passed each. No Linux run is claimed.

## Reproduction

All commands start in the repository. `tools/ct_schema.py command` acquires the shared fcntl lock and supplies the fixed target/jobs/auto-install environment.

- Build each variant: `python3 tools/ct_schema.py command cargo +1.96.0 test --offline --locked --release --features tool-json --test ct_schema --no-run --message-format=json`. Copy the emitted executable to `~/.hermes/cache/scratch/ct-a` or `ct-b`, then `python3 tools/ct_schema.py identity <binary>` before changing production source.
- With both fixed binaries: `python3 tools/ct_schema.py collect`. This is the one bounded campaign, not an instruction to rerun after seeing results.
- Offline reduction: `python3 tools/ct_schema.py reduce`.
- Allocation A: baseline production sources, `python3 tools/ct_schema.py command cargo +1.96.0 test --offline --locked --release --features tool-json --test ct_alloc allocation_diagnosis -- --exact --ignored --nocapture`.
- Allocation B: same command on B, or `-- --include-ignored --nocapture` to run the width-invariance regression too.
- Exact errors: `python3 tools/ct_schema.py command cargo +1.96.0 test --offline --locked --release --features tool-json --test ct_errors -- --nocapture`. Compare the `CTERROR` lines against the committed baseline `errors-a.txt`.
- Suites: `python3 tools/ct_schema.py command cargo +1.96.0 test --offline --locked`, then add `--features tool-json`, then `--release --features tool-json`.

The ordinary release remains non-PGO. No parser/path/integer redesign, SIMD, kernel DSL, Tool specialization or website change is included.

## Separate compiler-profile registration correction

The follow-up diagnostic-only commit adds parse/type measurements to `parse_validated_tools` and an enclosing frontend measurement to native `compile_with_environment`. The two declaration-based native wrappers have the same missing outer measurement and receive the same correction. Parsing time is recorded even when AST parsing fails; type time is recorded for semantic failures. Schema/source-boundary failures before parsing do not claim parser work. Callers still reset the thread-local profile before each diagnostic; these fields are not an accumulating service telemetry API.

All clocks and profile updates are guarded by `compiler-profile`. The ordinary build performs no new timer reads. Parser/type timers are nested within the native frontend interval and must not be added to frontend as disjoint phases. Tape's shared frontend now reports parse/type, but the native-only outer `frontend_ns` remains zero on tape. Two focused tests assert populated/nested success timers for all three Tool entry points and zero unreached backend phases for parser/type/schema errors. They passed with `tool-json,compiler-profile`; the ordinary release focused regression (both JSON variants, owned plans, allocation width, exact errors, authorization and fused CO) also passed after this change. Actual diagnostic values in `profile-tests.txt` are separate from formal request results; no phase-share estimate is constructed.

Command: `python3 tools/ct_schema.py command cargo +1.96.0 test --offline --locked --features tool-json,compiler-profile --test ct_profile -- --nocapture`. Ordinary focused command: `python3 tools/ct_schema.py command cargo +1.96.0 test --offline --locked --release --features tool-json --test ct_schema --test ct_alloc --test ct_errors --test tool_environment --test co_count`.
