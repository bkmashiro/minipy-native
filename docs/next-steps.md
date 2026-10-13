# Next bounded optimization lane

D has now delivered the script entry and four measured replacements. Use
[optimization-roadmap.md](optimization-roadmap.md) for the single current pointer
and [experiments.md](experiments.md#d-script-entry-and-bounded-performance-iteration-2026-10-14)
for D's evidence. The sections below are C's historical handoff, not a new task
list. The owner representation/API was subsequently authorized under unattended
engineering discretion, measured on Linux and migrated in `af06948`; its old
pending-approval wording below describes the C snapshot.

C's integrated evidence is in [performance-v2.md](performance-v2.md), with raw
Linux data at `bench/results/c-linux-v2/`. Measured runtime/harness snapshot is
`065c7cb`; final publication adds report/figures, not another runtime optimization.
The operator already created D `t_f435e8a1`, dependent on C. Do not create another
continuation card or rerun the entire C matrix as an orientation exercise.

## Highest-value unblocked candidate: source-cold compilation

Observed on the idle ARM64 VM: weighted 100-row cold total 209.29 us, compile
198.29 us; stateful 100-row total 291.00 us, compile 278.96 us. Ordinary Monty
cold totals are 117.63 and 118.38 us. At 10,000 rows native's compile attribution
is 198.33/286.17 us out of 289.50/382.92 us. These are different median columns,
not an additive accounting equation. Removing B's diagnostics is already adopted.

First profile only a representative zero/100-row fresh compile to distinguish
parse, lowering, backend, ISA/module construction and code release. Source
inspection: `src/codegen.rs:65-80` creates settings, the native ISA, JIT builder,
module and context on every compile. A simple hypothesis is to reuse immutable
host/Tool-ABI material across source-cold requests, without caching source, IR,
compiled user logic or task state. Its contribution is unmeasured. Select one
piece only after profiling; compare total cold CPU/wall and live code/memory.
The latency break-even model is not proof that the workload has reusable source.

Read earlier negative singlepass/regalloc and verifier results before retrying
those routes. Preserve trusted lowering, RX ownership, import validation,
capability authorization, checked arithmetic and total/chunk resource bounds.
No safety verifier removal without a specific redundant invariant proof.

## High-impact owner candidate: experiment pending API decision

B's exact patch is `docs/b-performance-v2-evidence/owner-candidate.patch`. It
changes `Arc<[Quote]>` to `Arc<Box<[Quote]>>`, avoids the exact-capacity Vec-to-Arc
payload copy, and frees the boxed payload when the last strong owner drops even
if Weak observers remain. `src/buffer.rs:27-36` is the current copy site. Source
and argument production must stay inside request timing. `into_boxed_slice` may
shrink spare capacity; do not call all import paths zero-copy.

Existing B M4 evidence: 1M rows, warm compiled source, total 1.423 -> 1.057 ms
pooled medians; requested peak about 32 MB -> 16 MB, with a slower first candidate
round retained. C did not merge or remeasure this candidate on Linux. C's physical
memory probe has no Weak observer, so its deallocation/RSS result must not be
used as a claimed weak-only allocation result.

Affected public surfaces are `QuoteBuffer::downgrade()` (`src/buffer.rs:87`) and
`TestProvider::last_rows()` plus its storage (`src/provider.rs:45`, methods/tests).
They currently expose `Weak<[Quote]>`; the candidate exposes `Weak<Box<[Quote]>>`.
The explicit typed observer in boundary tests also changes. Recommendation:
approve this narrow representation/API change if the prototype observer API can
move, rather than introducing a generic compatibility owner framework. Until
approved, keep the candidate isolated. A bounded Linux probe may verify full
producer/import/cleanup cost, requested peak, second wait and RSS before asking
for final adoption; this decision need not block independent compile work.

## Waiting-task fixed overhead and a language gap

The full zero-row first-wait diagnostic has a requested-layout slope around
5.36 KiB/task native and 48.51 KiB/task Monty. Native's frame alone is 280 bytes,
so this is not a frame-size result: providers, detached operations, task handles,
semaphore and diagnostic collection also contribute. Before changing scheduling,
isolate one allocation contributor. Compare mixed long/short p99 plus total CPU
and detached cancellation reclamation. Do not replace Tokio or build an allocator
pool on the basis of this diagnostic slope.

Fixed cold arrivals show native short p99 2.520 ms ready / 7.983 ms with two
1-ms sleeps; Monty 18.409 / 57.451 ms. These are pooled finite samples from three
process runs, not an SLO. The reactor still performs synchronous compilation;
its admission path is not a parallel compiler pool. Monty `quotes=[]` reduces
logical row retention but only about 2.3% of requested VM allocation at record
wait, so representation/capacity deserves attention more than a missing `del`.
Pinned Monty rejects `del`; reuse C's clear control instead of retesting it.

The 10B native Vec diagnostic has no compiler/task/language path. It cannot accept
a string feature. A real bytes/string slice requires decisions on value type,
UTF-8 validation, ownership/view retention and Tool ABI; no syntax/encoding/API
expansion is approved by C. Propose the smallest contract after existing hotspots
converge, rather than implementing it to make the storage number look complete.

## Handoff limits

Source-cold timing excludes process launch, includes fresh source/compile/input
and cleanup, and uses ready or finite real timer waits, not external service
traces. Memory counters are requested layouts; sampled RSS/PSS and cumulative
observed HWM are separate measurements. Raw reported VmHWM is retained because
kernel accounting can lag. No production multi-tenant/untrusted-native isolation,
automatic cache, general Python support, or exact worst-case peak claim exists.
