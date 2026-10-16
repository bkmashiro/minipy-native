# DG correctness and representation evidence

Reviewed base: `4fc5564` (DF). No performance measurement.

- `validation.json`: mechanically summed Cargo test-result lines (separate
  invocations overlap; do not sum entries as unique tests).
- `default.log`: full default release, 84 passed / 3 ignored.
- `json.log`: full tool-json release, 187 passed / 13 ignored.
- `debug.log`: json_array/json_get/co_count/debug_runtime with both features,
  31 passed / 0 ignored.
- `arrays.log`: 12 array integration tests, each runtime test uses real native
  and tape, plus current concrete execution-future sizes.
- `native-cfg.log`: actual retained source-dependent CLIF, not a simulated kernel.
- `layout.log`, `no-array-layout.log`: actual rustc sizes, with named regression
  assertions. Baseline Op/State/unit/304-byte no-array tape were also observed
  before production edits via the existing bx_diagnostic; its preparation timing
  output is not used as a benchmark. Before-schema shapes are compiler-laid-out
  beside the candidate to avoid claiming a cross-commit timing comparison.
- `oracle.json`: five CPython valid-input executions of unchanged CV04.
  Duplicate-key and invalid-JSON policies are tested in Rust, not conflated with
  CPython's dict construction. `identity.json` records fixture/source SHA256,
  CPython version, target path and rustc identity.
- `clippy.log`, `fmt-check.log`, `diff-check.log`: focused verification success.
- `clippy-all-targets-failure.log`: strict broad lint attempt stopped on existing
  ct_schema findings; they were not edited or suppressed. This is not an
  all-target lint-clean result.

Cargo commands are recorded at the start of every log. All used the existing
external target, `CARGO_BUILD_JOBS=1`, `RUSTUP_AUTO_INSTALL=0`, Rust 1.96.0 and
`--offline --locked --release`, under a nonblocking exclusive fcntl lock on
`/Users/yuzhe/.hermes/cache/scratch/minipy-perf-build.lock`. Mount/capacity and
lack of concurrent Cargo/rustc were checked first. No new target/worktree,
download, CI, Linux/Wasm run, latency/Monty campaign or resampling.

Full semantics, explicit costs, API migration and reproduction:
[`docs/dg-arrays.md`](../../../docs/dg-arrays.md).
