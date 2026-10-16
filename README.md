# MiniPy Native

An in-process research prototype for running a small Python subset as Host Tool
glue over typed data. Scripts run on a computation tape or Cranelift-generated
native code. Start with tape for short, one-off scripts; native is an explicit
option for prepared reuse or longer scans. Hosts select the backend, with no
automatic tiering or source cache.

## Example

From [fixtures/weighted_script.py](fixtures/weighted_script.py):

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

The Host declares inputs and Tools. Calls execute in source order, suspending at
asynchronous Host waits. Each task returns status and private stdout. Earlier
output survives a later error; the last expression has no implicit echo.

## Run

Requires Rust 1.93 or newer. The demonstration CLI uses native code and injects
`batch_id=7` and `price_limit=700` with a delayed test provider:

```sh
cargo run --release -- fixtures/weighted_script.py
```

Add `--ir` after the filename to inspect generated Cranelift IR. This CLI uses
fixture data; it does not connect to an external Tool service.

## Mechanisms

- Static binding resolves names, fields and Tools before execution through a
  shared typed frontend.
- Scans read immutable typed rows without per-row objects. Opt-in `tool-json`
  reads declared paths on demand through borrowed views, decoding escaped strings
  when needed without building a full object tree.
- Continuations preserve live state across Tool waits and bounded scan-chunk
  yields. Cancellation wakes waiting tasks.
- Last-use analysis releases buffer and JSON owners when no later operation needs
  them. Detached provider work retains its required arguments until completion.

## Supported subset and limits

Top-level scripts support i64/bool locals, checked arithmetic, comparisons,
short-circuit conditions, sequential Tool calls, Quote scans and scalar `print`.
Builtins include `len(quote_buffer)`, checked `abs(i64)` and two-argument i64
`min`/`max`. Integers are checked signed-64, not Python arbitrary precision.

With `tool-json`, Hosts can declare JSON leaves, bounded string Tool arguments
and small object-array scans (64 KiB, 256 visited elements per loop) with static
i64 fields. Declared paths accept dot or literal-subscript selectors.
String leaves support literal containment,
equality and prefixes, bounded composition at print/Tool sinks, limited f-strings
and literal-key `.get()` string defaults for missing final keys. Direct
`for line in text.splitlines():` scans a declared string leaf using CPython line
boundaries, with loop-only borrowed
lines, literal containment/prefix tests, print, scalar accumulation and `break`.

No general Python objects, imports, functions/classes, packages, dynamic lookups,
general collections or string-valued locals. Nested loops, `continue`, for-else
and Tool calls inside loops are unsupported. See the
[current generation guide](docs/code-mode-guide.md) and the detailed
[string defaults](docs/df-get.md), [object arrays](docs/dg-arrays.md) and
[borrowed lines](docs/dr-lines.md) contracts for restrictions and bounds.

Task budgets bound scanned rows, retained buffers, Tool calls and stdout.
Compiler, runtime and providers form the trusted computing base.
Providers must authorize each Tool call and bound detached work. Task budgets
provide no service-wide quota or wall-clock preemption guarantee.

## Library entry points

- Native: `codegen::CodeUnit::compile_script` and `runtime::execute_script`.
- Tape: `interpreter::InterpreterUnit::prepare_script` and `interpreter::execute_script`.

With `tool-json`, a resident Host can validate declarations once with
`tool::ToolEnvironment::new` and use either unit's `*_with_environment` entry point.

The [Host routing example](docs/cy-routing.md) prepares the whole source before
execution and leaves unsupported-source routing to the caller. Syntax, type,
schema and preparation resource errors are rejected. Runtime failures never
trigger fallback or replay. No external Python executor is included.

## Documentation

- [Design and semantic contract](docs/design.md)
- [Current generation guide](docs/code-mode-guide.md)
- [Experiments and reproducibility](docs/experiments.md)
- [Implementation and handoff](docs/implementation.md)
- [Optional Rust-host PGO build and bounded results](docs/cu-pgo.md)
- [Browser Wasm tape and debugger API](docs/playground-engine.md)

## License

[MIT](LICENSE).
