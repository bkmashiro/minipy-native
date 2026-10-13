# MiniPy Native

A research prototype for compiling short Python-like agent programs into native code over typed data, with bounded execution and asynchronous Host calls.

**Status: working bounded prototype, with real macOS and Linux ARM64 execution and reproducible synthetic comparisons. Initial implementation and focused integrated evidence reviews completed; cost/density results are available.** Working name; independent of Pysolate's CPython/Wasm runtime.

## Goal

Minimize source-to-result CPU, data movement, waiting-state memory, and code/runtime footprint. Prefer eliminating serialization, materialization and unnecessary work to optimizing their overhead. Target high-density, short-lived, I/O-heavy execution without assuming code-cache hits.

## First slice

Rust + Cranelift; Linux ARM64 acceptance, local macOS development. Reuse a parser and async I/O infrastructure when practical. Compile a small typed language, not CPython and not arbitrary Python.

```python
async def main(batch_id: i64, price_limit: i64) -> i64:
    quotes = await tools.load_quotes(batch_id)
    total: i64 = 0
    for q in quotes:
        if q.quantity > 0 and q.unit_price_cents <= price_limit:
            total += q.unit_price_cents * q.quantity
    return await tools.record_total(batch_id, total)
```

This syntax is executable. Load an immutable typed buffer asynchronously, scan it without per-row objects, release it, then await an effectful call returning a small receipt.

## Run

Core: Rust 1.93 or newer. No Monty dependency in the runtime crate.

```sh
cargo test
cargo run --release -- fixtures/weighted_scan.py
cargo run --release -- fixtures/stateful_branch.py --ir
cargo clippy --all-targets -- -D warnings
```

The demonstration provider performs two real delayed asynchronous operations. The source file determines the generated scan; its input rows are a small provider fixture. `--ir` prints the actual Cranelift IR. Errors exit unsuccessfully rather than recording a partial total.

Comparisons live in a separate `bench/` crate. It needs Rust 1.96 and pins Monty to an upstream commit containing a fix missing from its published 1.1.0 package. Reproduction commands and limitations are in [Experiments](docs/experiments.md). At 10,000 rows the weighted native cold path beats ordinary Monty, but loses to Monty plus a parameterized typed-buffer filter/product/sum helper. That finite primitive does not support the cross-row stateful variant. Earlier fixture-specific Rust helpers remain handwritten performance floors. These results do not decide the value of general compiled user logic or justify assuming cache reuse.

## Implemented boundary

RustPython parsing → typed scan IR → Cranelift → checked native loop, with a current-thread Tokio dispatcher. The accepted program has a fixed load/scan/record effect skeleton, `i64` and `bool` scalars, checked arithmetic, comparisons, short-circuit booleans and cross-row locals. No arbitrary post-loop statement block is supported. Quote/v1 rows are immutable 16-byte records. Compilation and providers remain trusted components.

Engineering bounds are 16 KiB of source, 512 checked IR/frontend nodes, depth 32, 32 scalar slots including two parameters, one million imported rows and chunks of at most 4,096 rows. These are prototype choices, not platform standards or a general CPU/time quota. Default chunk size is 256. Cancellation wakes suspended tasks; detached operations retain their own arguments until completion. Quote/frame ownership and the task's code reference are released before record I/O.

- [Design and semantic contract](docs/design.md)
- [Experiments and redesign record](docs/experiments.md)
- [Implementation mission and handoff](docs/implementation.md)
- [Integrated cost, density and finite-load evidence](docs/performance-v2.md)
- [Bounded follow-up candidates and pending API decisions](docs/next-steps.md)

No packages, dynamic imports, generic object model, automatic result replay, or cross-task fusion in the first slice. Code and mutable task state are separate. Native lowering remains part of the trusted computing base.
