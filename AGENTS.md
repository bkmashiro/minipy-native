# Engineering rules

Read README.md and docs/{design,experiments,implementation}.md first. This is a new private research project, not a Pysolate refactor.

## Priorities
- Aggressive performance and density are the goal. Minimize performed work, copies, allocations, retained memory, compilation, and dispatch overhead.
- Keep one small executable vertical slice. Prefer a single crate with modules initially. No framework, plugin system, distributed scheduler, package ecosystem, generic query optimizer, automatic cache or extra metadata pipeline without measured need.
- A design is a hypothesis. If a simpler or faster design is plausible, record the hypothesis and run a bounded experiment. Adopt measured improvements that preserve the behavior/authority contract; record negative results and remove dead alternatives. Ask before changing public semantics or the threat model.
- Checks must protect a named invariant. Validate immutable inputs at import, amortize allocation/budget checks, eliminate or hoist proven-redundant checks. No per-field authorization, per-row hashing, repeated full-buffer validation or ceremonial defensive layers.
- Do not remove spatial/lifetime safety, capability authorization, resource bounds or effect error handling to improve a score. Prove redundancy before removing a check. Assertions and test-only diagnostics need not run on the production hot path.
- No unsafe untrusted execution claims: compiler/lowering, runtime and trusted providers form the TCB. Ordinary user errors must become task errors, not worker crashes.

## Work and evidence
- Lead: gpt-6.1-sol on openai-codex. Use gpt-6-luna delegates for bounded independent lanes with exact API/file ownership, tests, and handoffs. Lead owns architecture, integration and performance conclusions. Do not delegate the entire project unchanged.
- Verify the actual delegate model, never silently substitute. Delegate children do not close the parent Kanban card. Keep worktree/module ownership explicit.
- Use targeted tests proportional to risk, plus end-to-end acceptance. Do not build a governance framework or endless review loop. Request one focused independent review at a concrete integrated commit.
- Measure full costs against a strong Monty baseline. Distinguish source inspection, synthetic measurements and real workloads. No fabricated traces, mocked native execution, hard-coded business kernel replacing compilation, or moving costs outside the timed interval.
- Commit reproducible small raw results, commands and source/toolchain identity. Ignore build trees, downloaded dependencies, large artifacts and credentials. Do not install CI workflows by default.
- Signed commits and push to the existing private origin after validation. No force pushes, changes to Pysolate, paid infrastructure, Docker, or global system/kernel changes. Existing local development VM use is allowed; inspect status/capacity and avoid concurrent unrelated experiments. Gateway restart is user-only.
- Checkpoint progress, limitations and commands in docs/experiments.md and the Kanban card. If environment is blocked, continue native macOS correctness work; label Linux validation pending rather than claiming a cross-build is a run.
