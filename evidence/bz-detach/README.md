# BZ: large JSON root / small async string argument

Decision: **keep the demonstrated lifetime mechanism as an isolated patch; defer
its production copy policy**. Final `src/` and manifests equal base `f311832`.
No API, persistent state, scheduler or default copy behavior is added. The BT
adapter retains two opt-in cases, with its original four defaults unchanged.

## Stage synthesis and next slice

These are prior experiments, not new BZ measurements:

- BV deferred validated-input schemas and the two architectural candidates.
  Two short names cost about 84.6 ns isolated vs 3.567 us full tape preparation;
  this did not establish worthwhile request-level savings for a public API.
- BW kept synchronous joined/f-string borrowing: one real allocation disappears
  for the escaped joined sink (16 to 15), without a stable causal E2E win.
- BX reverted generic checked multiply-add fusion. Op widened 56 to 64 B for all
  instructions and prepared loop capacity grew 120 B; request results were mixed.
  Production has no fused tape. This does not reject all possible representations.
- BY reverted its explicit Ready candidate: two allocations removed but both
  execution futures grew 216 B, including Deferred; no consistent request win.
  It yielded after **every** Ready. This is not a test of amortized allowance-based
  direct continuation and does not refute that different mechanism. Do not rerun
  BY here or weaken fairness to rescue its measured candidate.
- BZ below proves large-root retention can be removed at a bounded copy cost,
  with no stable request latency win in these synthetic ready operations. Unlike
  BW, this adds work in exchange for retention, so its threshold needs a workload.

Next concrete slice is the already-created CB (`t_18a3bb4f`) isolated single-target
Copy-and-Patch feasibility experiment. Prior BT native diagnostic attributes
78.7% median backend/prepare cost (historical, not new BZ timing). The source
shows each fresh native unit still runs define_function/backend. Removing that
work is a larger, distinct hypothesis than redoing schema/Ready/fusion negatives.
CB must compare current **unfused** tape and Cranelift with real templates,
installation, checked arithmetic and cleanup included, no cache assumption, one
bounded design and no production backend expansion. BZ establishes no C&P result.

## Candidate and invariants

`candidate.patch` adds one synchronous private helper and calls it at direct JSON
string Tool captures in both engines, using existing continuation root liveness.
It first runs the original string_value validation/decode/scan charging. Only an
unescaped validated string whose root has no future consumer can be copied.
The experimental engineering thresholds are root logical length >= 64 KiB,
leaf <= 4096 B. They are not a standard, measured workload optimum or wait-time
prediction. Sliced/shared Bytes may have larger invisible backing; length is
not a physical-capacity estimator. The copy itself has independent ownership.

The existing available-byte calculation includes live task roots, decoded
strings and external Quote buffers. Insufficient transient leaf headroom falls
back to the original owned view, not a new Budget error. Thus the candidate does
not reduce the budget to manufacture a retention result. No extra scan charge
is imposed: validation/search fuel is unchanged, and extra copy work is bounded
by the private leaf cutoff. The original root remains in State until the existing
post-authorization release. Error evaluation precedes authorization as before.
Cancellation, capability checks, call debit, spawn/oneshot, operation ownership,
reply kind/buffer checks, permits and effects stay in their original order.

Escaped strings already own a decoded allocation and need no root copy. Joined
strings already materialize and are outside this candidate. Root aliases and
nested views use the existing root bit. A continuation consumer suppresses copying.
No future, frame, StringCache, plan or instruction field changes. No await occurs
inside the helper, no speculative operation poll and no new unsafe code.

## Separate backing-owner diagnosis

`baseline-diagnostic.json` and `candidate-diagnostic.json` each record one actual
release libtest process, with 12 controlled waits: six cases on both backends.
The source returns a fresh Bytes::from_owner holding an exact Box<[u8]>. Its Drop
explicitly deallocates the Box before publishing an atomic release marker. There
is no other backing owner. This proves backing deallocation at the observation
point, not merely a shorter Bytes slice or reference-count change. It does not
prove allocator pages were returned to the OS or an RSS/density improvement.

For the dead-root case, baseline waits with 1,048,621 payload bytes in the argument;
candidate deallocates that backing before operation entry and owns a 10 B copied
leaf. Source and returned owner lengths bound/report the copied payload, not an
allocator-call census. Actual allocator rounding, Bytes control blocks and heap
metadata are excluded. No allocation-count claim is made. Both retain an equally
sized concrete test wrapper future: 984 B (union of both backend futures plus
test captures, **not** separate execution future sizes).

Live continuation, tight budget, 5000 B leaf and small root keep their original
backing in both arms. Escaped strings release the large root already on baseline.
Nested paths/root aliases are exercised throughout. Each guest is then cancelled:
its result is Cancelled while the operation remains blocked; after the gate opens,
the detached operation reads the correct parameter and drops it, including the
copied case. Uncopied backing deallocation is asserted only after that completion.
Full existing suites additionally cover argument/permission/error ordering, fuel,
multiple roots, stdout budgets and real HTTP permits/late completion.

An initial test used reserved IDs 1/2 and failed admission; 101/102 corrects the
fixture without changing admission. Initial diagnostics observed Weak<Arc<[u8]>>:
upgrade failure proves strong-owner release but the Weak itself retains allocation.
Those preliminary files are retained and **not used** for backing-memory claims.
The Box+Drop correction above reran the complete candidate suites and both
ownership diagnostics. Formal binaries/timings were unaffected by this test-only
correction. The final Python opt-in-case test initially lacked the two new names;
its explicit expected list was updated, keeping default cases unchanged.

## Formal resident fresh-source requests

macOS 26.4.1 ARM64, rustc 1.96.0. Both arms use the identical extended BT adapter,
16 resident declarations, registered environment and I/O-enabled runtime. One
unused declaration is changed to the string-consuming numeric-result fixture in
both arms. Scalar/Quote are controls. `detach` constructs a new JSON body with
1 MiB padding inside every request, accesses a small leaf and passes it to a
second owned ready operation; `detach_live` additionally prints the leaf later.
This synthetic request is deliberately favorable to retention; it is not evidence
of prevalence or actual provider wait distribution. No real HTTP/Monty benchmark.

Timing includes fresh-source preparation/native code generation and installation,
actual execution, producer allocation, Tool dispatch, exact stdout/status checks,
and all unit/output/task/machine-code cleanup before stop. Caller source string
construction and separately reported startup are outside. No prepared-unit reuse,
phase timer, counting allocator or owner instrumentation in formal binaries.

Each archive is two ABBA rounds, **64 successful subprocesses / 2048 measured
requests**, 32/process after sample 0. A/A runs the exact A executable in both
arms. Medians below are medians of per-process medians, never pooled samples:

| backend / case | A ns | B ns | A/B B:A | A/A B:A |
|---|---:|---:|---:|---:|
| tape detach | 61177.00 | 62250.00 | 1.017539 | 0.997792 |
| tape live | 62656.25 | 63083.50 | 1.006819 | 1.004534 |
| tape scalar | 2947.75 | 2666.50 | 0.904588 | 0.882061 |
| tape Quote | 12166.50 | 12313.00 | 1.012041 | 0.985186 |
| native detach | 138969.00 | 140239.50 | 1.009142 | 0.997638 |
| native live | 146875.25 | 146416.75 | 0.996878 | 1.015630 |
| native scalar | 42364.25 | 41968.50 | 0.990658 | 0.973378 |
| native Quote | 117531.25 | 114927.00 | 0.977842 | 0.980975 |

Startup medians: A/B archive 74.6045 us, A/A 71.8335 us, not per-request savings.
Raw preserves strong early drift: first tape scalar A/B process medians are
13.563/10.146 us; later values are much lower. A/A scalar also moves by 11.8%.
Separate A/A is not a statistical noise bound. Both detach cells are slightly
slower; do not attribute these small changes solely to the copy or claim a
causal regression. No stable latency benefit justifies a default copy policy.
The waiting-payload reduction remains a valid bounded mechanism result.

## Reproduction and validation

Run from the repo with mounted `target` symlink to
`/Volumes/M.2/caches/minipy-native-target`. Driver/verification scripts hold
`~/.hermes/cache/scratch/minipy-perf-build.lock`, set CARGO_BUILD_JOBS=1 and the
single existing CARGO_TARGET_DIR, and use offline/locked Rust 1.96.0 builds.
No dependency download, additional target/worktree, campaign or CI.

```
python3 tools/perf_workbench.py build --output "$HOME/.hermes/cache/scratch/bz-new-a"
git apply evidence/bz-detach/candidate.patch
python3 tools/bz_verify.py evidence/bz-detach/new-candidate.json candidate
python3 tools/perf_workbench.py build --output "$HOME/.hermes/cache/scratch/bz-new-b"
python3 tools/perf_workbench.py run --baseline "$HOME/.hermes/cache/scratch/bz-new-a" --candidate "$HOME/.hermes/cache/scratch/bz-new-b" --cases scalar quote detach detach_live --rounds 2 --reps 32 --output evidence/bz-detach/new-ab.jsonl.gz
python3 tools/perf_workbench.py run --baseline "$HOME/.hermes/cache/scratch/bz-new-a" --aa --cases scalar quote detach detach_live --rounds 2 --reps 32 --output evidence/bz-detach/new-aa.jsonl.gz
# Restore only the candidate production changes; leave bz_detach.rs for baseline diagnostic.
git apply -R --exclude=tests/bz_detach.rs evidence/bz-detach/candidate.patch
python3 tools/bz_verify.py evidence/bz-detach/new-baseline.json baseline
# Remove the experimental test before ordinary baseline/final suites.
python3 tools/bz_detach_reduce.py
```

Use fresh output names; no raw is overwritten. Formal manifests retain binary
hashes, source hashes, toolchain, base commit, exact commands and tracked diff
identity. `candidate.patch` archives the full private regression/diagnostic test.
`candidate-box-verification.json` is passing corrected candidate full default and
tool-json/debug-runtime suites. `final-verification-fixed.json` records passing
restored-source full suites, strict all-target Clippy (installed Rust 1.93.1),
rustfmt (installed 1.93.1), nine Python workbench tests and diff check. Rust 1.96
has no installed rustfmt; no component was downloaded. No Linux/Wasm performance
or system-memory measurement is claimed. Temporary fixed executables are removed
at handoff; the external shared build target is retained.
