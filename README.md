# MiniPy Native

An in-process research prototype that runs a small Python subset over typed data using a computation tape or Cranelift-generated native code.

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

The Host declares inputs and Tools. Calls execute in source order, suspending at asynchronous Host waits. Each task returns status and private stdout. Earlier output survives a later error, and the last expression has no implicit echo.

## Run

Requires Rust 1.93 or newer. The demonstration CLI uses native code and injects `batch_id=7` and `price_limit=700` with a delayed test provider.

```sh
cargo run --release -- fixtures/weighted_script.py
```

Add `--ir` after the filename to inspect generated Cranelift IR.

## Mechanisms

- Static binding: parsing and type checking resolve names, fields and Tools to slots before execution. Both backends share the typed frontend.
- Direct data access: scans read immutable typed rows without per-row objects. Opt-in `tool-json` accesses declared JSON paths on demand through borrowed views, decoding escaped strings when needed rather than building a full object tree.
- Suspension and resumption: continuations preserve live state across Tool waits and bounded row-chunk yields. Cancellation wakes waiting tasks.
- Last-use ownership: liveness analysis releases buffer and JSON owners when no later operation needs them. Detached provider work retains its own required arguments until completion.

## Supported subset and limits

Top-level scripts support inferred i64/bool locals, optional annotations, checked arithmetic, comparisons, short-circuit conditions, sequential Tool calls, Quote scans and scalar `print`. Builtins include `len(quote_buffer)`, checked `abs(i64)` and two-argument i64 `min`/`max`. Integers are checked signed-64, not Python arbitrary precision.

With `tool-json`, Hosts can declare JSON leaves and bounded string Tool arguments. String leaves, literal concatenation and limited f-strings work at print/Tool sinks. String predicates support literal `in`, declared string leaf `==` / `!=` literal (either side), and `leaf.startswith(literal)`, including existing leaf aliases. Equality and prefixes compare decoded Unicode without normalization. Prefix/equality literals are bounded to 4096 UTF-8 bytes. Tuple prefixes, start/end arguments, general string-valued locals and arbitrary string operations are unsupported.

Declared JSON selectors accept `data.content` as an alias for `data["content"]`,
including nested/mixed paths, existing view/leaf aliases and declared array-row
i64 fields. Both forms bind the same path and share caches and runtime behavior.
Nonidentifier keys require subscripts. Bare `data.get` selects a declared `get`
field; `data.get("key", "default")` still means the existing method. Tool calls
and `startswith` methods are unchanged. No general object attributes, dynamic
lookup or field mutation are provided; unknown fields fail preparation.

With `tool-json`, `for line in text.splitlines():` scans a declared JSON string
leaf or its existing alias on both backends, using CPython line boundaries.
The borrowed loop-only view supports literal `in line`, `line.startswith(literal)`,
`print(line)` and existing print joins/f-strings, with scalar accumulation and
`break`. No list result, arguments/keepends, line equality/rebinding/escape,
nested loops, `continue`, for-else or Tool calls inside loops. Existing token,
scan, cumulative rows fuel and chunk bounds apply, without the array 256-element
cap. See [contract and public enum migration](docs/dr-lines.md) and the
[current generation guide](docs/code-mode-guide.md).

Limited f-strings also consume declared JSON i64 leaves directly, for example
`print(f"ready: {state['count']}")`. Fields evaluate left to right at the sink;
format specifiers, conversions and arbitrary embedded expressions remain unsupported.

Declared string paths also accept `.get(key_literal, default_literal)` at these
same sinks, including concatenation and limited f-strings. Only a missing final
object key uses the default. Present null/wrong types and invalid receivers fail.
Candidate missing objects receive bounded UTF-8/structural validation, while
present/required access retains its lazy behavior. See [semantics, costs and the
public StringSource migration](docs/df-get.md).

Default iteration uses the fixed Quote/v1 layout (`unit_price_cents`, `quantity`).
With `tool-json`, Hosts can also opt into one declared object-array path per Tool,
with static i64 fields, sequential `for` and `break`. This slice bounds selected
arrays to 64 KiB and visited elements to 256 per loop, using the existing task
fuel, aggregate buffer cap and chunk cancellation. See [array semantics, costs
and API migration](docs/dg-arrays.md). General collections/record schemas, nested
scans, Tool calls inside loops, conditional buffer creation, Quote buffer aliases,
general functions/objects and packages remain unsupported.

Execution bounds cover scanned rows, retained buffers, Tool calls and stdout. Chunk limits apply per scan, not per task CPU slice. Compiler, runtime and providers are trusted components. Providers must authorize each Tool call and bound detached work. Task budgets do not impose a service-wide memory or in-flight-operation quota.

## Library entry points

- Native: `codegen::CodeUnit::compile_script` and `runtime::execute_script`.
- Tape: `interpreter::InterpreterUnit::prepare_script` and `interpreter::execute_script`.

Hosts choose the backend explicitly. There is no automatic tiering or source cache. With `tool-json`, a resident Host can validate declarations once with `tool::ToolEnvironment::new` and use either unit's `*_with_environment` entry point.

The [Host routing example](docs/cy-routing.md) prepares the whole source before
execution and leaves unsupported-source routing to the caller. Syntax, type,
schema and preparation resource errors are rejected. Runtime failures never
trigger fallback or replay. No external Python executor is included.

## Documentation

- [Design and semantic contract](docs/design.md)
- [Experiments and reproducibility](docs/experiments.md)
- [Implementation and handoff](docs/implementation.md)
- [Optional Rust-host PGO build and bounded results](docs/cu-pgo.md)
- [Browser Wasm tape and debugger API](docs/playground-engine.md)

## License

[MIT](LICENSE).
