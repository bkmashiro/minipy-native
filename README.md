# MiniPy Native

A research prototype for compiling short Python-like agent programs into native code over typed data, with bounded execution and asynchronous Host calls.

**Status: working bounded prototype, with real macOS and Linux ARM64 execution and reproducible synthetic comparisons. Initial implementation and focused integrated evidence reviews completed; cost/density results are available.** Working name; independent of Pysolate's CPython/Wasm runtime.

## Goal

Minimize source-to-result CPU, data movement, waiting-state memory, and code/runtime footprint. Prefer eliminating serialization, materialization and unnecessary work to optimizing their overhead. Target high-density, short-lived, I/O-heavy execution without assuming code-cache hits.

## Script entry

Rust + Cranelift; Linux ARM64 acceptance, local macOS development. Reuse a parser and async I/O infrastructure when practical. Compile a small typed language, not CPython and not arbitrary Python.

```python
quotes = tools.load_quotes(batch_id)
total = 0
for quote in quotes:
    if quote.quantity > 0 and quote.unit_price_cents <= price_limit:
        amount = quote.unit_price_cents * quote.quantity
        total += amount
receipt = tools.record_total(batch_id, total)
print(receipt)
```

This syntax is executable. Host declares and injects the two scalar inputs and `tools`. Calls retain their ordinary sequential meaning while actual Host waits suspend asynchronously. Native scans use immutable typed rows without per-row objects. The task returns status, private stdout and error. Earlier stdout survives failure; the last expression has no implicit echo. The old annotated `async def main` benchmark source remains supported through the same typed IR and code generator.

## Run

Core: Rust 1.93 or newer. No Monty dependency in the runtime crate.

```sh
cargo test
cargo run --release -- fixtures/weighted_script.py
cargo run --release -- fixtures/scalar_script.py
cargo run --release -- fixtures/weighted_scan.py
cargo run --release -- fixtures/stateful_branch.py --ir
cargo clippy --all-targets -- -D warnings
```

The demonstration Host injects `batch_id=7`, `price_limit=700`. Its provider performs real delayed asynchronous operations; scripts without a Tool have no provider call. The source determines generated computation, not a recognized business template. `--ir` prints actual Cranelift IR. Errors exit unsuccessfully and preserve earlier task output. Library Hosts can use `CodeUnit::compile_script(source, input_schema)` and `runtime::execute_script` for a declared schema, including zero inputs.

Library Hosts may explicitly use `InterpreterUnit::prepare_script` and `interpreter::execute_script` instead. This lightweight typed-IR path shares the frontend, typed buffers, Tool authority, cancellation, budgets, checked arithmetic and bounded stdout contract; it skips native compilation and executes a sealed computation tape. The CLI stays native. There is no automatic backend choice, source cache or hot-loop JIT. Cold small scripts favor interpretation; prepared reuse and long scans can favor native. Integrated mixed-cost measurements, waiting memory and a reverted local optimization are in [H average-cost experiment](docs/average-cost-experiment.md).

With the opt-in `tool-json` feature, a Host may declare a `JsonTyped` string leaf and print it directly, or through an unconditional leaf alias, on either backend: `data = tools.fetch(batch_id); print(data["message"])`. The existing borrowed/decoded JSON view supplies UTF-8; the whole line including the print newline must fit the task's remaining stdout byte budget. The Provider must still authorize the Tool ID; a declaration alone grants no access. A separate opt-in `ToolDeclaration` with one `ArgumentSchema::String("path")` accepts a bounded literal or a declared JSON string leaf as a Tool argument: `data = tools.read_file("report.txt"); print(data["content"])`. Use `compile_with_declarations` / `prepare_with_declarations` and implement `Provider::call_bound_tool` to handle `ToolArguments::String`; the default denies strings and forwards numeric calls to the old `call_tool`. The Host must recheck path, permissions, reply bounds and detached-work budgets. No generic Python strings, dynamic paths, global `read_file` alias or real filesystem Host integration are provided. See [BO evidence](docs/experiments.md#bo-bounded-print-of-an-existing-declared-json-string-leaf-2026-10-15) and [BQ evidence](docs/experiments.md#bq-unified-bound-string-tool-argument-2026-10-15).

The current interpreter lowers the typed IR directly to one sealed computation tape with static leaf operands, checked destination writes and short-circuit comparison branches. It retains no boxed expression tree after preparation. Scratch temporaries live only in synchronous turns, outside waiting futures. [N paired qualification](docs/compact-interpreter-experiment.md) records the new version's costs and small-input/tail regressions; the frozen L cross-engine figures describe the earlier production version.

Comparisons live in a separate `bench/` crate. It needs Rust 1.96 and pins Monty to an upstream commit containing a fix missing from its published 1.1.0 package. Reproduction commands and limitations are in [Experiments](docs/experiments.md). At 10,000 rows the weighted native cold path beats ordinary Monty, but loses to Monty plus a parameterized typed-buffer filter/product/sum helper. That finite primitive does not support the cross-row stateful variant. Earlier fixture-specific Rust helpers remain handwritten performance floors. These results do not decide the value of general compiled user logic or justify assuming cache reuse.

## Implemented boundary

RustPython parsing → typed semantic IR → native CFG with resume points at Tool waits and row-chunk yields, with a current-thread Tokio dispatcher. Top-level statements support inferred signed-64 integers and bools, checked arithmetic, comparisons, short-circuit conditions, naturally introduced scalar locals, sequential Tool calls/scans and scalar print. Direct `len` of a loaded Quote buffer, checked-i64 `abs` and two-argument i64 `min`/`max` are supported; string/JSON length and iterable overloads are not. With the opt-in `tool-json` Host schema, a declared i64 Tool call may also be a standalone expression statement (including the existing top-level `await` spelling); its effect is retained and only its result is discarded. Names are statically bound, not looked up at runtime. Ordinary optional `int`/`bool` annotations are accepted. Quote/v1 rows are immutable 16-byte records. Compilation and providers remain trusted components. This remains a small Python subset: no nested scans, Tool calls inside loops, conditional buffer creation, mutable buffer aliases or general functions/objects/strings.

Engineering compile bounds are 16 KiB of source, 512 frontend nodes, depth 32, 32 scalar slots including declared inputs and 32 statically bound buffers. Each imported buffer is capped at one million rows. Default task limits are one million cumulative scanned rows, 32 MiB of live buffer bytes, 512 Tool invocations, 16 KiB stdout and chunks of at most 4,096 rows (default 256). These are prototype choices, not standards or a general CPU/time quota. Integers are checked signed-64, unlike Python arbitrary precision. Cancellation wakes suspended tasks; detached operations retain their own arguments until completion. Buffer leases drop at proven last use. Code remains live across waits if subsequent computation or print needs it; terminal legacy record retains its early code/frame release.

- [Design and semantic contract](docs/design.md)
- [Experiments and redesign record](docs/experiments.md)
- [Ablation summary and static-work ledger](docs/ablation-summary.md)
- [Implementation mission and handoff](docs/implementation.md)
- [Opt-in browser Wasm interpreter and Worker/debugger API](docs/playground-engine.md)
- [Integrated cost, density and finite-load evidence](docs/performance-v2.md)
- [Frozen end-to-end benchmark, Python lifecycles and figures](docs/benchmark-overview.md)
- [Execution reference and bounded strategy qualification](docs/stage-strategy-experiment.md)
- [阶段性能结论与使用建议（中文）](docs/stage-performance-conclusion.md)
- [Bounded follow-up candidates and pending API decisions](docs/next-steps.md)
- [Current optimization pointer and historical decisions](docs/optimization-roadmap.md)

D's current results and cost trade-offs are in [Experiments](docs/experiments.md#d-script-entry-and-bounded-performance-iteration-2026-10-14). Small source-cold scripts still lose to ordinary Monty. General continuations increase cold compilation cost and code footprint; prepared execution and large typed workloads improve. The observer API is now `QuoteWeak = Weak<Box<[Quote]>>`, and scalar input mismatch uses `TaskError::Input`. Explicit old Weak types or exhaustive error matches need migration.

No packages, dynamic imports, generic object model, automatic result replay, or cross-task fusion in the first slice. Code and mutable task state are separate. Native lowering remains part of the trusted computing base.

Chunk limits apply to each scan, not an entire task CPU slice. Task-local budgets and cancellation cleanup do not provide a global quota for in-flight or cancelled detached Provider operations. Provider construction is outside the post-receive buffer cap; see [scheduling and admission limits](docs/design.md#native-execution-and-scheduling).

Final frozen-version evidence includes actual Linux ARM64 MiniPy codegen/typed IR, ordinary Monty, CPython spawn/resident/prewarm+fork and Node/Pyodide resident lifecycles. Common source bodies and a checked handwritten AArch64 reference are recorded separately. [Overview figure](docs/figures/benchmark-overview.svg) and [Python lifecycle figure](docs/figures/benchmark-python-lifecycles.svg) use current-round raw only; cold preparation, explicit reuse, finite-window overload and waiting PSS have distinct boundaries.
