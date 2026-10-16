# DL validation evidence

Baseline: d7b0936. Actual source identities are the SHA-256 maps in each record,
not an assertion that uncommitted work was present in baseline HEAD.

- `checks-initial.json`: initial fixture failures, retained. Literal string print
  requires a declared Tool environment; the synthetic test omitted one. The
  permission/cancellation check was changed to the existing scalar path and the
  UTF-8 test now declares an unused Tool. No production admission change.
- `checks.json`: first passing targeted run and example build.
- `final-checks.json`: final source hashes, 28 targeted Rust tests, targeted Clippy,
  five Python tests, stable fmt/diff, and wrapper commands. Clippy retains the
  documented existing `clippy::type_complexity` allowance for frontend.rs.
- `smoke-timing.jsonl`, `smoke-diagnostic.jsonl`: the ten existing public synthetic
  fixtures on both real backends at the explicit uniform 262144-byte cap. Each
  mode has 20 results, all matching its synthetic expected outcome, including
  intended rejections and runtime errors. These are not 20 successful tasks.
- `zero-quota.jsonl`: same synthetic fixtures at cap 0. All 20 stdout values are
  empty; intended print attempts fail Budget. Admission/earlier Tool effects are
  not undone. Correctness against the original smoke expectations can be false.

Every wrapper output records policy, input/source/toolchain/binary identities.
Timing values are incidental validation observations, not performance evidence.
Diagnostic mode is a separate execution. No model generation, private task
replay, real Tool effects, Linux or Wasm validation is represented here. Private
v3 diagnostic/prompt regressions and the Chinese handoff remain exclusively in
the authorized private corpus directory.
