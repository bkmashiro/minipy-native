# CB: bounded real Copy-and-Patch feasibility gate

Decision: **defer a source backend; keep only this isolated feasibility evidence**.
No production code, public API, dependency, default backend or main branch change.
The tested installer can run compiler-produced checked arithmetic without calling
Cranelift's function compiler. Its optimistic per-request cost already consumes
about the entire measured tiny tape request. This is a reason to stop this design
for very short one-shot glue, not evidence that Copy-and-Patch or larger loops
cannot win. No further campaign or new allocator is scheduled.

Base: `0a23facd7910ac1f49e569bb4b412012dbac8e6a`, experimental branch
`exp/cb-copy-patch`. BX removed multiply-add fusion; the control is the actual
current **unfused** tape. BZ left production unchanged. CA's historical backend
attribution motivated this experiment, but is not subtracted from these timings.

## What actually ran

Existing Apple clang 21.0.0 compiled `stencils.c`, target `arm64-apple-macos14`,
with the recorded optimization/no-stack/no-unwind flags. No download, assembly
emitter, handwritten arithmetic instructions, source cache or fixture-result table.
`templates.json` records the command, compiler identity, C/object hashes, symbol
boundaries, bytes, continuation relocation offsets and full objdump output.
The object is 576 B; its four text templates total **100 B**: add/sub 28 B each,
mul 36 B, return 8 B. No non-text payload/literal pool or helper relocation exists.
Object extraction uses the installed objdump for inspection and stdlib Mach-O
parsing for bytes/metadata. `llvm-readobj` is unavailable; it was not installed.

Each arithmetic template is a complete C-ABI leaf. It reads two trusted adjacent
i64 slots, does checked arithmetic, returns status 1 on overflow without storing,
or stores its result and uses compiler `musttail` continuation linkage. This
avoids stripping guessed prologues or stitching ordinary calls. `BR26` is the
only supported object relocation, with external `_next`, zero addend and the
expected compiler branch word. Request preparation copies templates and patches
only that branch's displacement after alignment, extent, opcode and signed-range
checks. Arithmetic opcodes themselves come directly from clang. It is real
Copy-and-Patch of continuation linkage, but **not yet arbitrary source lowering**.
The C compiler and trusted extraction/patching code are in the TCB.

Pinned cranelift-jit/module 0.128.3 exposes `Module::define_function_bytes`.
Its implementation copies a CompiledBlob and queues finalization without
`Context::compile`/`define_function`. The prototype uses that API, the same
JITBuilder/ISA facilities as production, then `finalize_definitions` and
`get_finalized_function`. No new executable allocator. The default system
provider clears the instruction cache, applies RX protection, flushes the pipeline,
and `free_memory` unprotects/deallocates. This is the existing local JIT policy;
no hardened-runtime/MAP_JIT entitlement, tenant isolation or other platform claim.
Finalization/declaration errors return through an owner that also calls
`free_memory`; a bare JITModule Drop would leak. Function pointers never escape
the synchronous owner. The external target remains on the mounted M.2 volume.

Correctness ran 363 boundary operand pairs (121 each for checked add/sub/mul),
covering MIN/MAX, negative/zero/positive and overflow without destination overwrite.
Two stitched add→mul cases verify first-error exit and preservation of an earlier
successful write. Invalid patch metadata is rejected. These are real machine-code
calls compared against Rust checked arithmetic, **not** tape/native differential
coverage for a completed source backend. There is no task replay, await, Tool,
loop, cancellation, capability or resource-policy implementation in this raw gate.
The fixed four-slot state is internal; it is never exposed as a guest pointer.
A private 16-template bound prevents unbounded synchronous copying/execution.

## Installation stop gate, not an E2E engine speedup

`tests/cb_gate.rs` holds three explicitly unequal arms:

- `tape`: each fresh `x = a + <changed constant>; print(x)` is parsed, typed,
  prepared, executed with the existing scalar input/provider/limits, checked and
  fully released before timer stop.
- `native`: the same source, inputs, provider, limits and stdout/status checks,
  including real Cranelift compilation/install/execute/cleanup.
- `install_floor`: preselected add+return templates, stitch/patch, ISA/module
  creation, copy/install, one checked machine-code call, result check and full
  module/code cleanup. No parsing, typing, source matching, constants lowering,
  Output, scheduler or task protocol. Its second operand is a host-filled slot,
  **not** a source-dependent patched constant. It cannot count as compilation of
  the corresponding Python. This intentional optimistic arm tests whether even
  the remaining installation work fits the tiny-request budget.

Caller source construction and input preparation are outside all timers; existing
runtime/provider infrastructure is process-resident. No profile/allocator/phase
instrumentation in these samples. No subtraction of unrelated prepare timings.
The 24-process archive uses two bounded, reversed-order ABBA-mode rounds, 64
measured requests per process, with sample 0 excluded: **1536 measurements**.
The same fixed executable is checked before/after every subprocess. All builds
and timing hold the shared fcntl lock, offline/locked Rust 1.96.0, one Cargo job.

Medians are medians of eight per-process medians:

| arm | ns |
|---|---:|
| full fresh-source tape | 3542.00 |
| optimistic raw installation floor | 3989.25 |
| full fresh-source Cranelift | 42437.50 |

Floor/tape is 1.1263, floor/native 0.0940. **Do not present the latter as a C&P
speedup**: substantial source/runtime work is deliberately absent. Raw preserves
warmup/drift: floor process medians decline 5000→3687.5 ns, tape's first is 5666 ns
then mostly ~3542 ns. Even the late floor is close to the entire tape path, rather
than establishing headroom for the omitted source and runtime work. The bounded
choice is defer; this does not establish a causal 12.6% regression or a universal
installation lower bound across other allocators/implementations.

A separate same-binary, same-tape A/A archive has two ABBA rounds, **8 processes /
512 measured requests**, B/A 0.97918, with visible early drift. A/A is a control,
not a statistical confidence/noise bound. No repeat-to-significance or second
stencil design. `summary.json` and `aa-summary.json` reduce the committed raw.

## Memory and unbuilt work

Measured template payload is 100 B, process-resident in the test binary; it is
reported separately from request code. The timed add+return request code is 36 B;
the stitched add→mul test is 72 B. State is exactly four i64 slots, 32 B, held on
the synchronous caller's stack. Host page size is 16384 B; inspected pinned
SystemMemoryProvider rounds an executable allocation to a page, so one 36 B
function occupies at least one 16 KiB code allocation in this design. This is a
source-derived allocation bound, not an RSS/physical-density measurement. Module,
ISA, declaration tables, allocator metadata, staging Vec capacities and binary
text/rodata are extra and not censused. No async future/waiting-state improvement.
No new metadata was added to ordinary tape instructions.

Moving from this proof to a safe source backend still requires independently
patchable operands (clang currently pairs adjacent loads with LDP), constants,
expression temporary/layout selection, branches/short circuit, loop chunk exits
and the existing error/output/Host boundary integration. Templates do not yet
implement that. Since the cheap install gate consumes the tiny path's budget,
those costs were **not** hidden in setup or implemented as a third compiler to
rescue a score. The requested full source C&P, short-circuit, loop-size and
Tool/JSON-control matrix was deliberately **not reached**. Large-loop value,
source-dependent machine-code variation and source/backend error-order equivalence
remain untested. Linux/Wasm, Monty, real workloads and prepared-only speed claims
are outside this result.

The demonstrably avoidable work is invoking the generic function compiler to
install this tiny trusted region. It has not been shown that removing that work
improves complete user requests over tape or preserves full production generality.
Keep production unchanged. Any future wider design needs a concrete workload or
a separate installation design with measured headroom, not an assumed reuse rate.

## Reproduction and verification

Read commands in `templates.json` and manifests. Run from the repository with
mounted `target -> /Volumes/M.2/caches/minipy-native-target`:

```
python3 tools/cb_templates.py
python3 tools/cb_gate.py
python3 tools/cb_gate.py --aa
python3 tools/cb_verify.py
python3 -c 'from tools.cb_gate import reduce,OUT; print(reduce(OUT/"cost-gate.jsonl.gz")); print(reduce(OUT/"aa.jsonl.gz"))'
```

The timing driver refuses to overwrite raw. For a new execution preserve/move the
old evidence first or use a fresh checkout of the experiment before adding raw.
The Rust test is experimental-only on this branch, not production/API integration.
`prototype.patch` preserves its source/tooling additions relative to the base;
raw/templates/docs are small retained evidence. Temporary object and gate binary
are deleted. First Rust build needed an explicit Result error type; no semantics
or tests were weakened. Focused review and final validation are recorded separately.
