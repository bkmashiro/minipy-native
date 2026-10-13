# Minimal design contract

## Objective and scope

Serve many small Python-like programs with near-direct native data access and small waiting states. The intended gains come from avoiding JSON round trips, dynamic object construction and needless buffer retention, then from native user logic. No claim of advantage over Monty exists yet.

One program execution is one logical task. Filter/map/fold and basic blocks are compiler structures, not separately scheduled actors. Start with ordinary for loops; generator syntax, Pipe ownership rules and explicit materialization are later features, not prerequisites.

## Language

Start with typed parameters, i64/bool, local variables, structured conditions/for loops, checked arithmetic, fixed record field reads, return and explicit await of declared tools. Reject unsupported syntax clearly. No package installation, dynamic imports, eval, reflection, runtime class mutation, arbitrary pointers, recursion or mutable closures. Static namespaces such as tools are permitted. Do not add two equivalent syntax forms for convenience before evidence warrants it.

Signed i64 overflow is a task error. Conditions short-circuit. Input order determines multiply/add order and first data error; no reassociated reductions. A filtered row must not trigger arithmetic in the skipped branch. An empty scan returns zero. Resource errors terminate the task. Concrete source/IR size and execution limits should be few, documented engineering choices, not guessed production SLOs.

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

Frozen owner lifetime covers all reads. A tiny view retaining a large owner may justify a small copy. Last-use release should drop quotes before the second await. Do not confuse dropping a lease with reducing RSS: pool and provider retention stay accounted.

On cancellation, an outstanding operation keeps its necessary arguments alive until it really completes or acknowledges cancellation. Late completion validates task generation/operation identity, releases its resources, and never revives a cancelled or reused task slot. Cancelled outstanding operations retain resource charges. Code stays alive while a suspended task can resume.

## Native execution and scheduling

One task entry returns Call, Yield, Done or Failed. Use a small iterative dispatcher, one Ready queue initially and a mature async I/O runtime. Do not execute unrestricted CPU loops in the I/O reactor. Record waiting state before a provider can complete to prevent lost wakeups.

A native scan keeps position and accumulators in registers where possible and spills at actual yield, not every row. Save input owner, row index, sum and necessary parameters across CPU yield. Clear input owner before record_total.

Start with correct budget/cancellation checks, then compare bounded chunks against row checks. Fixed row charging is suitable only while per-row work is bounded. Long helpers must also have bounded work or an explicit execution domain. Await/yield never resets the task's total budget. CPU yield must preserve all live calling state; avoid supporting arbitrary suspendable nested calls until needed.

Compiler frontend emits restricted accesses, not arbitrary loads/stores. Centralize safe lowering and backend memory flags. Explicit overflow/bounds/resource failures follow cleanup and return a task error; do not use native faults as normal errors. RX/RW does not isolate tenants by itself. Side-channel resistance is out of scope; unauthorized memory access is not.

## Deferred ideas

A useful experiment may justify adopting them later: canonical component reuse, pipeline syntax, SIMD/batching, view-connected workflows, bounded handoff, sharing scans of an explicitly authorized immutable version. Do not implement these to rescue a negative first benchmark. No world-change journal, automatic result repair, distributed scheduler or general fusion engine here.
