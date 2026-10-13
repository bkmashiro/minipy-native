# Implementation mission

## Owner and scope

A Kanban worker pinned to gpt-6.1-sol (openai-codex) leads this prototype and dispatches gpt-6-luna children. Default profile delegation is configured to Luna; verify worker/subagent run metadata. Sol owns design decisions, contracts, integration, experiments and final evidence. Children receive narrow tasks and cannot self-certify final integration.

Do not automatically adopt all ideas in prior discussions. The controlling contract is this repository's design plus the user's performance objective. Do not expand the language merely to accommodate a convenient benchmark.

## Execution order

1. Establish the simplest buildable Rust layout and small IR/task/buffer interfaces. Pin compatible dependency versions after inspecting MSRV; local initialization observed Rust 1.93.1, not a promise that latest dependencies support it. Prefer project-local toolchain configuration over changing global defaults. Select the smallest suitable existing parser; do not build a parser framework.
2. After interfaces exist, delegate independent bounded work to Luna. Suggested split: typed frontend + lowering tests; buffer/provider/task lifecycle tests. Sol handles native codegen and integrates, or reassigns boundaries based on source evidence. Separate worktrees or non-overlapping modules; no blind concurrent edits to shared Cargo manifests/contracts.
3. Run the ordinary-for vertical slice end-to-end with real Cranelift machine code and async provider. Unsupported syntax is an explicit compile error. Use a tiny reference loop/interpreter only as test oracle, not a second product VM.
4. Exercise essential correctness/lifecycle boundaries; run actual Linux ARM64 acceptance. Existing local development VM is authorized, but inspect current availability and capacity first, coordinate with unrelated work, and do not require historical Pysolate build artifacts. Continue macOS implementation while Linux prerequisites are resolved. No Docker/paid cloud/global kernel changes.
5. Measure the quote case and a stateful variant against a pinned Monty baseline; record compile/execute/waiting-memory costs. Pursue a small number of profile-supported aggressive optimizations or redesigns with matched evidence. Do not turn a negative result into an unbounded new language project.
6. Signed integrated commit, focused independent review on a fixed snapshot, fixes if needed, local validation, push to private origin and verify remote SHA. Request native same-card Kanban review; no duplicate review graph or automatic CI. Report limitations, evidence and remaining decisions.

## Delegation hygiene

Pass exact files/API contracts, semantics, expected tests, command paths, forbidden scope and completion evidence to each Luna child. Keep a short progress comment before long runs and at stage changes. Read child diffs and run integration tests. Subagents should return commit/path handles, commands and actual outputs; do not accept self-reported success alone. Do not modify other Hermes profiles or global model settings.

No requirement to manufacture many subagents: two useful independent lanes after the contract is enough. Do not split every module into a separate card or make Sol merely forward messages.

## Research context

This project follows Pysolate investigations but does not reuse CPython/Wasm as its execution path. Pysolate remains untouched. Monty is the strong alternative, not a strawman. Previous inspection pinned Monty main at 56949a4c45f51cedc7a96450782f58a8017e114d; use a pinned runnable compatible version and report differences. It already has inline scalar values, arena storage, async suspension and snapshots.

Reference sources (verify versions when used):
- https://cranelift.dev/
- https://pydantic.dev/docs/monty/quickstart/rust/
- https://github.com/pydantic/monty
- https://docs.rs/cranelift-codegen/latest/cranelift_codegen/ir/struct.MemFlagsData.html

No performance conclusions are inherited from Pysolate benchmarks. No arbitrary 20% gate; identify break-even regions and full-system cost. A project-specific decision record belongs in docs/experiments.md, not in global agent memory.

## Integrated handoff

The first native slice is implemented in one runtime crate with a small demonstration binary. The independent benchmark crate keeps Monty and its newer toolchain out of the runtime dependency graph. Source programs and reproducible raw results are committed in `fixtures/` and `bench/results/`; commands and cost limitations are recorded in `docs/experiments.md`.

Thirty tests pass on macOS and on the actual Linux ARM64 VM. Integration additionally corrected cancellation during pending provider I/O and releases code after its final native call. The two useful Luna lanes in the resumed integration run were acceptance testing and an initial benchmark implementation; actual returned identities were gpt-6-luna/openai-codex. Sol corrected and completed the benchmark against pinned upstream Monty, added the stronger typed host-helper comparison, and reran the integrations and measurements. No other project or Hermes profile was modified.

Remaining gate: one same-card focused independent review of the signed integrated commit. Review unsafe load/call bounds and code lifetime, checked expression/error order, cancellation versus late completion, pre-record owner release, and whether the cost report matches the raw measurements. Do not reopen architecture, expand syntax, run an unbounded campaign or add CI. The measured source-once path loses to the finite host-helper alternative on both sources; this negative result is part of the deliverable, not a reason to manufacture a cache/reuse assumption.
