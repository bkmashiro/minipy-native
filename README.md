# MiniPy Native

A research prototype for compiling short Python-like agent programs into native code over typed data, with bounded execution and asynchronous Host calls.

**Status: design and implementation brief only. No working compiler or performance claim yet.** Working name; independent of Pysolate's CPython/Wasm runtime.

## Goal

Minimize source-to-result CPU, data movement, waiting-state memory, and code/runtime footprint. Prefer eliminating serialization, materialization and unnecessary work to optimizing their overhead. Target high-density, short-lived, I/O-heavy execution without assuming code-cache hits.

## First slice

Rust + Cranelift; Linux ARM64 acceptance, local macOS development. Reuse a parser and async I/O infrastructure when practical. Compile a small typed language, not CPython and not arbitrary Python.

```python
async def main(batch_id: i64, price_limit: i64) -> i64:
    quotes = await tools.load_quotes(batch_id)
    total = 0
    for q in quotes:
        if q.quantity > 0 and q.unit_price_cents <= price_limit:
            total += q.unit_price_cents * q.quantity
    return await tools.record_total(batch_id, total)
```

This is proposed syntax, not currently executable. Load an immutable typed buffer asynchronously, scan it without per-row objects, release it, then await an effectful call returning a small receipt.

- [Design and semantic contract](docs/design.md)
- [Experiments and redesign record](docs/experiments.md)
- [Implementation mission and handoff](docs/implementation.md)

No packages, dynamic imports, generic object model, automatic result replay, or cross-task fusion in the first slice. Code and mutable task state are separate. Native lowering remains part of the trusted computing base.
