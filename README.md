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

Comparisons live in a separate `bench/` crate. It needs Rust 1.96 and pins Monty to an upstream commit containing a fix missing from its published 1.1.0 package. Reproduction commands and limitations are in [Experiments](docs/experiments.md). At 10,000 rows the weighted native cold path beats ordinary Monty, but loses to Monty plus a parameterized typed-buffer filter/product/sum helper. That finite primitive does not support the cross-row stateful variant. Earlier fixture-specific Rust helpers remain handwritten performance floors. These results do not decide the value of general compiled user logic or justify assuming cache reuse.

## Implemented boundary

RustPython parsing → typed semantic IR → native CFG with resume points at Tool waits and row-chunk yields, with a current-thread Tokio dispatcher. Top-level statements support inferred signed-64 integers and bools, checked arithmetic, comparisons, short-circuit conditions, naturally introduced scalar locals, sequential Tool calls/scans and scalar print. Names are statically bound, not looked up at runtime. Ordinary optional `int`/`bool` annotations are accepted. Quote/v1 rows are immutable 16-byte records. Compilation and providers remain trusted components. This remains a small Python subset: no nested scans, Tool calls inside loops, conditional buffer creation, mutable buffer aliases or general functions/objects/strings.

Engineering compile bounds are 16 KiB of source, 512 frontend nodes, depth 32, 32 scalar slots including declared inputs and 32 statically bound buffers. Each imported buffer is capped at one million rows. Default task limits are one million cumulative scanned rows, 32 MiB of live buffer bytes, 512 Tool invocations, 16 KiB stdout and chunks of at most 4,096 rows (default 256). These are prototype choices, not standards or a general CPU/time quota. Integers are checked signed-64, unlike Python arbitrary precision. Cancellation wakes suspended tasks; detached operations retain their own arguments until completion. Buffer leases drop at proven last use. Code remains live across waits if subsequent computation or print needs it; terminal legacy record retains its early code/frame release.

- [Design and semantic contract](docs/design.md)
- [Experiments and redesign record](docs/experiments.md)
- [Implementation mission and handoff](docs/implementation.md)
- [Integrated cost, density and finite-load evidence](docs/performance-v2.md)
- [Bounded follow-up candidates and pending API decisions](docs/next-steps.md)
- [Current D optimization pointer](docs/optimization-roadmap.md)

No packages, dynamic imports, generic object model, automatic result replay, or cross-task fusion in the first slice. Code and mutable task state are separate. Native lowering remains part of the trusted computing base.
