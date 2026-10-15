# AH: static Tool ABI and borrowed JSON slice

## Bounded contract (design before implementation)

Keep the existing frontend, typed semantic IR, sealed interpreter tape and Cranelift continuations. Existing scalar and Quote execution remains unchanged. Add an opt-in `tool-json` feature and Host-declared Tool compilation entry points. Tool names and named arguments resolve during preparation, never in the dispatcher. Each declaration has a unique numeric ID, at most two signed-i64 arguments, and either an i64 result or a JSON schema containing statically named signed-i64 leaf paths. IDs 0/1 remain the existing load_quotes/record_total adapters. Provider instances carry per-task capability authorization for the added IDs; declarations alone do not authorize calls.

Minimal source: `payload = tools.fetch(batch_id)`; `data = payload["data"]`; `n = data["count"] + 1`; `receipt = tools.publish(total=n, batch_id=batch_id)`; `print(receipt)`. Top-level Python assignments, indexing by literal field keys and scalar print, without new reserved variables. JSON subviews are statically bound root/path aliases, not a dynamic object type. No mutable containers, arbitrary string values, dynamic paths or language JSON-array loop in this first field-oriented slice. Sequential array parsing is separately exercised in the bounded parser A/B.

## JSON ownership and semantics

Use pinned sonic-rs safe APIs and bytes::Bytes. HTTP transfers a Bytes owner to the task without String/Value/dict conversion. A view is a root owner plus a checked raw range. Prefix/leaf caches are allocated only for JSON programs. Repeated field reads reuse the bound path slot; alias lifetime is the root's future-use lifetime, including across Tool waits. No DOM is retained. Only a reached expression requests its field, so branch short-circuit and left-to-right errors survive. No fused extraction of not-yet-demanded fields. Prefix and repeated leaf lookup are cached. Distinct sibling fields still scan their cached containing object with safe get; this slice does not claim a resumable object cursor or eliminate every sibling scan. The array experiment separately uses one cursor per row and two overwritten scalar slots, without retaining a DOM.

The access API is explicitly lazy, not json.loads: it may leave malformed unvisited suffixes undiagnosed. Requested missing fields, null, non-integer and out-of-i64 values are task errors. Selected raw integer tokens are parsed through sonic-rs, never unchecked access. Duplicate keys use the first matching key, including escaped spellings (tested). Escaped keys are decoded by sonic-rs internally; the original value remains a checked raw range. This differs from serde DOM last-key behavior and typed duplicate-field rejection. HTTP body limits protect parser work; full-document validation is not silently added at import. Borrowed subview ranges retain the root owner through their last use. Cancelled tasks drop their receive channel while detached operations retain their provider permit until actual completion. Late replies release their own Bytes, never revive execution.

## HTTP

Feature-gated Host Provider: one shared hyper-util pooled client with rustls using the existing Tokio runtime. Host binds Tool IDs to approved endpoints; user source receives no sockets or arbitrary URLs. Cap body bytes and in-flight operations. Preserve a single response chunk as Bytes; multiple chunks need one contiguous merge, with copy bytes reported. Single-chunk transfer copies zero payload bytes from provider to task (network transport allocation is not zero-copy networking). HTTP-free tasks instantiate no client. Keep admission permits in detached operations, not receiver lifetime.

## Files and verification

New modules: `src/tool.rs`, `src/json_view.rs`, `src/http_provider.rs`. Focused additions to `frontend`, `ir`, `codegen`, `runtime`, `interpreter/tape`, `provider`, `error`, manifest/lock, and optional debugger compatibility. New integration tests/fixture and one small example parser experiment. No bench/ frozen source, website, new backend, service runtime or general object library.

Verify compile binding, declared argument order/type diagnostics, real native/tape parity, requested JSON errors and skipped errors, subview retention across await, last-use release, cancellation/late response and permit retention, bounded body rejection, and localhost HTTP roundtrip. Run existing all-feature regressions. A/B: few-field, sequential array and repeated access, sonic lazy vs sonic typed deserialize vs existing serde_json, preparation+run and separate allocation/copy diagnostics, without trace. Report platform, independent repeats and raw measurements; no broad campaign or speed claim from unmatched semantics. Linux/WSL work is conditional on existing compute.lock availability; macOS evidence is labeled independently.

## Implemented public boundary

Enable native Host feature `tool-json`. `CodeUnit::compile_with_tools` and `InterpreterUnit::prepare_with_tools` accept the same source, scalar input schema and immutable `ToolSchema` list; execution uses the existing execute_script entry points and task limits. `Provider::allows_tool` defaults to denial and `call_tool` defaults to Permission. A Host authorizes IDs by constructing a per-task provider over an existing shared HttpService. The existing two Provider methods/IDs remain compatibility adapters and retain their independent load/record permissions. A declaration never grants authority. A scalar-only declared Tool does not allocate JSON state or import native JSON helpers.

    ToolSchema {
        id: 101, name: "fetch".into(), arguments: vec!["batch_id".into()],
        result: ResultSchema::Json(vec![vec!["data".into(), "count".into()]]),
    }

The script remains ordinary top-level restricted Python:

    payload = tools.fetch(batch_id)
    data = payload["data"]
    total = data["count"] + data["count"]
    receipt = tools.publish(total=total, batch_id=batch_id)
    print(receipt)

Named arguments are first evaluated into i64 locals in Python source order, then passed in declared positional order. Unconditional immutable JSON owners/subview aliases avoid ambiguous branch initialization and owner replacement. Assigned calls, static field keys and i64 leaves are this slice's complete new source capability. No general string values, array language loops, dynamic indexing, discard-only custom calls or nested Tool expressions are implied. Pure raw-subview assignment performs the demanded path check at that statement; its scalar contents are not converted.

A sealed native callsite returns status 8+callsite and writes up to two scalar arguments plus a statically typed target into existing Frame fields. No Frame growth or tool-name dispatcher is introduced. Tape uses the same bound call table and compact local-index operands. The JSON access helper writes an i64 through a compiler-owned Frame address, not a user pointer. Only the frontend can produce either sealed unit. The preparation plan is shared by Rc without per-task cloning of path Strings. Runtime state contains owner Bytes, bounded checked ranges and optional i64 cells. The owner has no borrowed self-references. Last-use masks drop roots before an ensuing Tool wait; aliases' future uses contribute their root bits. On cancellation, dropping the task state drops those roots independently of an in-flight operation's own state.

Engineering bounds: 32 custom declarations, 32 JSON roots, 128 bound prefix/leaf slots, 128 alias names, 32 keys/path and 256 UTF-8 bytes/schema key. Existing MAX_SOURCE_BYTES/MAX_LOCALS/MAX_NODES and Tool call budgets still apply. JSON and Quote owners share the existing aggregate buffer_bytes cap. These are slice-specific engineering limits, not sonic-rs or eBPF standards. Empty or oversized bodies are not imported as DOMs. Missing accessed fields, accessed null/string/bool/fraction/out-of-range integers, invalid UTF-8 and malformed selected subtrees fail with TaskError::Json. No integer coercion or unchecked API is used.

An unvisited malformed body can pass when the expression is short-circuited. Safe sonic get may also return a selected valid nested object even if its enclosing root lacks a closing terminator; this observed case is regression-tested. Locating a raw containing-object range may scan/validate that whole selected subtree, including syntax of its sibling values, while leaving an outside suffix undiagnosed. This is the lazy access contract, not full json.loads validation and not an equivalence claim on all malformed inputs. The cache does not request future branch fields or convert unvisited leaves.

## HTTP and runtime controls

HttpService uses one hyper-util 0.1.21 client, hyper 1.12.0 and hyper-rustls 0.27.10/rustls 0.23.46. It is constructed only by the Host on its existing Tokio runtime, with configured concurrency, body byte limit and operation timeout. Host-selected static endpoints determine URLs and result kind, numeric args travel in bounded x-minipy-arg headers for this fixture protocol. No redirect or arbitrary-source URL capability exists. HttpProvider has a finite per-task Endpoint table and uses numeric IDs. Limit failure rejects work rather than queueing unbounded operations. Body DATA metadata is additionally limited to 4096 chunks. Single-chunk responses retain the actual hyper Bytes directly; multi-chunk responses make one final contiguous merge. copied_bytes counts that merge, excluding transport/TLS/kernel work. No forwarding API is introduced, so this slice does not concatenate forward-only streams.

The shared pool must have an I/O-enabled Tokio driver for HTTP. Non-HTTP/ready-provider runtimes can enable_time without enabling_io. The old default crate feature set does not enable network dependencies. With tool-json enabled, calling Tokio Builder::enable_all also activates its reactor; on this macOS host, polling it across existing fairness yields adds measurable latency even to ready Quote tasks. The investigation and controlled raw data are retained in tool-json-results.md. Fairness yields were not removed or weakened to hide this cost.

Dependencies/API checked against the pinned downloaded sources: sonic-rs 0.5.7 Cargo.toml declares Apache-2.0, bytes 1.11.1 and hyper/hyper-util/rustls-family direct dependencies use permissive MIT/Apache licenses. No incompatible library is introduced. Safe get/to_array_iter/to_object_iter/from_slice are the only JSON parser APIs used. Current get carries a documented well-formed-JSON caveat; the tests characterize the lazy slice rather than promising a whole-document validator. References: https://docs.rs/sonic-rs/0.5.7/sonic_rs/fn.get.html, https://docs.rs/sonic-rs/0.5.7/sonic_rs/fn.to_array_iter.html, https://docs.rs/sonic-rs/0.5.7/sonic_rs/fn.to_object_iter.html .

## Verification and intentionally unimplemented capabilities

10 new Tool/JSON integration tests run both actual native and tape units; 4 tests use a real Tokio TCP fixture, including pooled keepalive, deterministic chunk split, byte caps, status errors, timeouts and admission retention after cancellation. Old all-feature tests, default-feature tests, strict clippy and existing Wasm/debug configuration remain validation gates. JSON/HTTP is a native Host feature and is not exposed by the legacy browser debug Engine.

No Linux/WSL numbers, HTTPS fixture campaign, full json.loads semantic equivalence, language array/container expansion, general Tool value types, source-level object cursor, automatic schema inference or proof of universal performance superiority is delivered. Distinct sibling scans remain a stated limitation; repeated fields and containing prefixes are cached. The one-pass array candidate is measured and deliberately not promoted because typed decoding is substantially simpler and faster on this input. The bounded field-oriented source/HTTP/owner slice is complete and submitted for review, not automatically merged or published.
