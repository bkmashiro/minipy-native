# Minimal design contract

## Objective and scope

Serve many small Python-like programs with near-direct native data access and small waiting states. The intended gains come from avoiding JSON round trips, dynamic object construction and needless buffer retention, then from native user logic. No claim of advantage over Monty exists yet.

One program execution is one logical task. Filter/map/fold and basic blocks are compiler structures, not separately scheduled actors. Start with ordinary for loops; generator syntax, Pipe ownership rules and explicit materialization are later features, not prerequisites.

## Language

The default source is a top-level Python statement script. Host declares and injects typed scalar inputs and the `tools` namespace. Source annotations are optional (`int`/`bool`); signed-64 integer and bool types are inferred from inputs, Tool schema and expressions. Ordinary assignment can introduce locals wherever definite initialization permits. Standard syntax is parsed by RustPython and lowered once into a small typed semantic IR. Names become scalar, buffer, row-field and Tool slots at compile time.

`tools.load_quotes(batch)` and `tools.record_total(batch, total)` have synchronous sequential source semantics. Their generated native continuations suspend at the actual asynchronous Host call without blocking a thread. There is no implicit parallelism, prefetch or effect reordering. Explicit `await` remains accepted for legacy source and lowers to the same operation. The old annotated `async def main` entry is an adapter to the same IR and native generator, not a separate scan implementation.

`print` takes one scalar argument and appends a line to task-private bounded stdout through a small native helper. It performs no Tool scheduling or process stdout I/O. Host receives status, stdout and error. Earlier output is preserved if a later computation, permission check, resource bound or Tool fails. Expression statements are evaluated and discarded, including the last expression; there is no implicit echo or designated result variable. Legacy `execute` continues to return its explicit integer result.

The implemented subset has sequential scans, scalar conditions and checked arithmetic. A loop-local value can be assigned and used in the same iteration; reading it after a possibly empty loop requires earlier definite initialization. Nested scans, Tool calls inside loops, conditional buffer creation, buffer alias/rebinding, recursive or arbitrary suspendable functions and non-scalar print formatting are explicitly unsupported. Reject unsupported syntax clearly. No package installation, dynamic imports, eval, reflection, runtime class mutation, arbitrary pointers, recursion or mutable closures. Bytes/string language values remain a separate unapproved contract.

Signed i64 overflow is a task error; integers differ from Python's arbitrary precision values. Conditions short-circuit. Input order determines multiply/add order and first data error; no reassociated reductions. A filtered row must not trigger arithmetic in the skipped branch. An empty scan leaves its preinitialized accumulators unchanged. Resource errors terminate the task. Concrete source/IR size and execution limits should be few, documented engineering choices, not guessed production SLOs.

## Initial Tool ABI

Deployment declares:

```
Quote/v1 = { unit_price_cents: i64, quantity: i64 }
load_quotes(batch_id: i64) -> awaitable<Batch[Quote]>
record_total(batch_id: i64, total: i64) -> awaitable<i64>
```

Quote/v1 uses 16-byte little-endian records, price at offset 0 and quantity at offset 8. This is a bounded prototype layout, not a universal wire format. Both fields accept every i64 bit pattern. Import validates authorized ownership, immutability, layout, real allocation range and overflow-safe length/stride arithmetic. Require alignment or use safe unaligned loads consistently with backend flags. Source programs cannot construct trusted descriptors.

Trusted same-process providers may freeze and lend buffers without serialization. External bytes cannot assert that they are trusted. A schema alone does not establish arbitrary JSON offsets. No JSON/Arrow adapter is needed for this first slice; both are future options, not fixed architecture.

Tool implementations stay behind declared capabilities; runtime authorization remains per task. Real asynchronous submission/completion is required, though a deterministic test provider is appropriate. No real trading or purchasing. record_total is effectful: failure before it prevents dispatch; no automatic retry. Cancellation after submission does not promise effect rollback.

## Memory and code

CodeUnit owns RX code, entry/state layout, source/error mapping and ABI identity. Task owns parameters, authorization, budget, continuation state and strong references to code and buffers. Code may be explicitly compiled once and used by multiple independent tasks; no automatic IR matching/cache required.

Use the smallest ownership mechanism that works. A process pool feeding task regions is a candidate, not a requirement to build a custom allocator first. Large response owners must be separately releasable. Mutable task data cannot overwrite runtime metadata. Views cannot expose arbitrary addresses or other tasks' data.

Frozen owner lifetime covers all reads. A tiny view retaining a large owner may justify a small copy. Last-use release should drop quotes before an await when no later scan needs them. Do not confuse dropping a lease with reducing RSS: pool and provider retention stay accounted.

The measured frozen owner is `Arc<Box<[Quote]>>`. Exact-capacity typed Vec storage is adopted without a payload copy; spare capacity may shrink and external-byte decoding still creates typed storage. A Weak observer retains the small Arc/Box header rather than the row payload after the last strong lease drops. `QuoteWeak` names this prototype observer API. Each task charges aggregate retained row bytes after import; provider construction/transient allocations and the complete process are not a hard memory quota. Private stdout grows geometrically up to its configured capacity bound, with whole-line append or failure preserving the earlier prefix.

On cancellation, an outstanding operation keeps its necessary arguments alive until it really completes or acknowledges cancellation. Late completion validates task generation/operation identity, releases its resources, and never revives a cancelled or reused task slot. Cancelled outstanding operations retain resource charges. Code stays alive while a suspended task can resume.

## Native execution and scheduling

One task entry returns Call, Yield, Done or Failed. Use a small iterative dispatcher, one Ready queue initially and a mature async I/O runtime. Do not execute unrestricted CPU loops in the I/O reactor. Record waiting state before a provider can complete to prevent lost wakeups.

A native scan keeps position and scalar locals in registers and spills at actual Tool suspension or chunk yield, not every row. The native entry has a compile-time-generated resume switch, not an IR interpreter. Runtime retains validated buffer leases and releases owners according to the continuation's future-use mask before submitting the next Tool. A later scan can intentionally keep a buffer across a record call. Code survives a record wait if later computation or print can enter it; a proven terminal legacy record still permits early code/frame release.

Start with correct budget/cancellation checks, then compare bounded chunks against row checks. Fixed row charging is suitable only while per-row work is bounded. Long helpers must also have bounded work or an explicit execution domain. Await/yield never resets the task's total budget. CPU yield must preserve all live calling state; avoid supporting arbitrary suspendable nested calls until needed.

Compiler frontend emits restricted accesses, not arbitrary loads/stores. Centralize safe lowering and backend memory flags. Explicit overflow/bounds/resource failures follow cleanup and return a task error; do not use native faults as normal errors. RX/RW does not isolate tenants by itself. Side-channel resistance is out of scope; unauthorized memory access is not.

## Deferred ideas
The explicit IR/native choices remain appropriate under the bounded
[I strategy experiment](stage-strategy-experiment.md). A feature-gated handwritten
reference qualifies fixed-fixture execution costs only, not unknown-source
compilation. Two local runtime candidates were reverted after end-to-end
qualification; neither automatic tiering nor a new backend is justified.


A useful experiment may justify adopting them later: canonical component reuse, pipeline syntax, SIMD/batching, view-connected workflows, bounded handoff, sharing scans of an explicitly authorized immutable version. Do not implement these to rescue a negative first benchmark. No world-change journal, automatic result repair, distributed scheduler or general fusion engine here.
