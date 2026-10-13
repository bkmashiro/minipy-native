# Experiments

Status: no measurements yet.

## Questions

1. Can a small typed frontend generate a genuine native loop with no per-row object allocation, Host callback or intermediate array?
2. How much does one-shot compilation add, and for which input sizes/user logic does native execution repay it?
3. Can the task release the response before its second I/O wait while preserving cancellation and late-completion correctness?
4. Where are CPU, memory, compile time and code size actually spent? Can a redesign eliminate work rather than merely tune it?

## Required comparisons

Use pinned Monty source/release identity. Compare ordinary Monty and Monty with the same efficient Host data layer and a predeclared finite set of general helpers. Do not force Monty through Python/IPC per field. If helpers accept an arbitrary execution plan, include its validation/compilation/execution cost.

Begin with the quote filter/aggregate fixture and add one bounded cross-row stateful/branching variant; simple aggregate results alone cannot decide the value of all compiled user logic. Parameters/conditions must change generated IR, not dispatch to a fixture-specific handwritten solution. Handwritten Rust with equivalent checks and ABI is an optional ceiling, not an equal sandbox claim.

Primary path: unique source execution, empty user-code cache, include parse/check/lower/codegen/install/execute/cleanup. Separately measure explicit compile-once/run-many without claiming an observed reuse rate. Include producer construction and any conversions. Delays are controls, not representative production wait distributions.

Measure small to large inputs, short/long scan mixtures and actual async completion. Count returned results and task errors. Report CPU, wall latency including queueing, compiled code, task/frame/operation bytes, physical owner capacity, peak and waiting retention. Include compiler, provider and pool resources. Result-return time and final release time differ. Do not require heavyweight per-load traces, universal percentage gates or a large benchmark framework.

## Optimization policy

First obtain a real trace/profile or structural attribution. Prefer redesigns that remove allocations, copies, materialization, checks or task state. Compare one change at a time with matched inputs and identical correctness contract. Report regressions and negative results. No disabled safety checks, weakened assertions or JSON-specific fixture tricks to manufacture speed. Compiler-proven check elimination is encouraged.

For the bounded record loop, compare per-row versus chunked checkpoints and record dispatch CPU, short-task tails and maximum observed cancellation latency. For source-once costs compare backend opt levels only if likely material. Expensive compiler work, caching and fusion need measured justification.

## Result entry template

Append compact entries below. Do not create a generic experiment database.

- Question / observed bottleneck:
- Proposed mechanism and preserved invariants:
- Source commit, platform/toolchain, exact commands and inputs:
- Matched raw result path and summary:
- Correctness tests and cost accounting:
- Decision: keep / revert / further bounded probe, and why:

## Acceptance

Real source parses and becomes machine code. Empty/filter/mixed/overflow cases match a simple semantic oracle; skipped rows do not fail; yields resume without duplicate work. Invalid descriptor/range, permissions, total budget, cancellation and late completion are covered by focused tests. Two actual asynchronous operations run, and a scan failure never dispatches the effectful operation. Waiting on the second operation does not retain the quote owner from the task. Multi-task code reuse does not share mutable state. Checks and limits protect explicit invariants.

Linux ARM64 execution is the target evidence. macOS native runs and cross-builds remain separately labeled. Fuzz/differential checks can be bounded and targeted; no claim that tests prove compiler security. Stop this milestone after the vertical slice, focused independent review and attributable cost report. A negative performance result is an acceptable research outcome, a stub is not.
