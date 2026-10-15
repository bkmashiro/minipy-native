# CD: one-pass forward-only tape owner analysis

Decision: keep the small preparation work elimination, with no E2E speedup claim.
Base `0e2caed168472f9f90d2b09dbe46ee0a3e29f95d`; macOS ARM64, Rust 1.96.0.
CC production policy and isolated CB branch `814fdd8` are preserved.

## Scope and proof

The supplied CA hypothesis is confirmed on current main by test-only counters:
tape buffer and JSON owner analysis can run a second reverse traversal solely
to confirm convergence. This does not apply to every analysis. Native `Live`
includes scalar assignment kills and loop-carried scalar values; its structured
fixed-point solver stays byte-for-byte unchanged. Tape scalars have no liveness
mask because their fixed task slots are always resident.

For strictly forward control edges, reverse PC order is a topological order.
At each instruction every successor mask is final, so the existing transfer
function computes the final mask in one pass, including branch joins, loads
that kill buffer slots, and Return (zero). The candidate notices backward/self
edges in the existing edge-transfer arms of that pass. If any exist, it uses
the old convergence condition and transfer functions without skipping a pass.
This is conservative for unreachable backedges and does not assume only Next
can point backward. No extra full scan, CFG, public API, persistent metadata,
allocation, task/frame field, dependency or runtime check is added.

`src/interpreter/liveness_tests.rs` keeps the pre-CD fixed-point algorithm as an
independent test oracle and compares every PC buffer/JSON mask for straight
code, branches, branch continuations, multiple owners, Tool string captures and
loops. Synthetic JSON edges cover backward Jump, Next, Branch, fused comparison
and Scan/self edges. Existing native-vs-tape differential and lifetime tests
cover scalar loop-carried state, errors/short circuit, real waits, cancellation,
late completion, string owners and CC budget fallback. Their unchanged native
analysis is a scalar/control reference, not an assertion that tape has an
unimplemented scalar analysis. Test counters compile out of integration/formal
binaries. No counters or compiler phase clocks are active in request samples.
An initial test fixture used unsupported script `return`; it was replaced by an
admitted branch continuation, without changing admission or production semantics.

## Actual visit counts (instructions processed, not timing)

| fixture | buffer old/new | JSON old/new |
|---|---:|---:|
| scalar (2 ops) | 2/2 | absent |
| straight buffer (7 ops) | 14/7 | absent |
| branch buffer (10 ops) | 20/10 | absent |
| branch + following buffer (8 ops) | 16/8 | absent |
| straight JSON (5 ops) | 5/5 | 10/5 |
| branch JSON (10 ops) | 10/10 | 20/10 |
| string argument wait (7 ops) | 7/7 | 14/7 |
| branch + following JSON (8 ops) | 8/8 | 16/8 |
| loop buffer (12 ops) | 36/36 | absent |
| loop JSON (12 ops) | 36/36 | 36/36 |

No-root JSON analysis was already skipped. All-zero buffer masks already
converged in one pass; those controls have no visit reduction. There is no
prepared-code/task memory or allocation reduction claim.

## Bounded request result

The existing BT workbench adds only two opt-in branch cases. Default cases stay
unchanged. A and B have identical adapter sources; the manifests differ only in
the two production liveness source files. A/A uses the exact A executable.
Each archive contains two ABBA rounds, 80 successful subprocesses, 3,840 measured
requests, and no failed/missing/status/stdout observations. Setup and sample 0
are excluded. Each measured request prepares fresh source, executes and checks
it, and releases output/code before the timer stops. Source construction is
caller work before the interval. Runtime, registration and loopback fixture
setup are separate startup. Ready JSON and Quote are synthetic Host fixtures,
not real filesystem/HTTP workload or Monty comparisons.

Ratio of arm medians of per-process request medians (B:A):

| backend | case | A/B | A/A |
|---|---|---:|---:|
| tape | scalar | 0.97699 | 0.99522 |
| tape | branch | 0.98455 | 1.01113 |
| tape | ready JSON | 0.95214 | 1.01643 |
| tape | branch JSON | 0.96708 | 1.02108 |
| tape | Quote loop | 1.00468 | 0.99908 |
| native | scalar | 1.02096 | 0.98145 |
| native | branch | 0.98948 | 1.00049 |
| native | ready JSON | 1.00576 | 0.99693 |
| native | branch JSON | 1.00189 | 1.00633 |
| native | Quote loop | 1.01277 | 0.95832 |

Untouched native and tape scalar controls drift; exact-binary A/A does too.
This bounded campaign finds no meaningful request regression, but cannot prove
small deltas causal or establish a speedup. Keep the certain removed traversal
at negligible local complexity. Do not repeat timing to chase the deltas.
No Linux, Wasm, RSS, density, throughput or Monty performance claim.

## Reproduction and verification

External `/Volumes/M.2` must be mounted and existing `target` must resolve to
`/Volumes/M.2/caches/minipy-native-target`. All builds and measurements used the
common fcntl lock `/Users/yuzhe/.hermes/cache/scratch/minipy-perf-build.lock`,
`CARGO_TARGET_DIR=/Users/yuzhe/projects/minipy-native/target`, one build job,
Rust 1.96.0 offline/locked. No new target/worktree/dependency/download/CI.

To reconstruct A, reverse only the production change in `src/interpreter.rs`
and `src/interpreter/tape.rs` (remove backward flag, edge comparisons and
`|| !backward`; retain cfg(test) counters/module). Keep the identical current
adapter and test oracle. For B restore the accepted production source. Build:

```
python3 tools/perf_workbench.py build --output /Users/yuzhe/.hermes/cache/scratch/cd-a
python3 tools/perf_workbench.py build --output /Users/yuzhe/.hermes/cache/scratch/cd-b
python3 tools/perf_workbench.py run --baseline /Users/yuzhe/.hermes/cache/scratch/cd-a --candidate /Users/yuzhe/.hermes/cache/scratch/cd-b --cases scalar branch ready branch_ready quote --rounds 2 --reps 48 --output evidence/cd-acyclic/ab.jsonl.gz
python3 tools/perf_workbench.py run --baseline /Users/yuzhe/.hermes/cache/scratch/cd-a --aa --cases scalar branch ready branch_ready quote --rounds 2 --reps 48 --output evidence/cd-acyclic/aa.jsonl.gz
python3 tools/cd_acyclic_reduce.py
```

Build/run driver locks itself. Raw output paths must not already exist. For
serial counters, take that same fcntl lock, set shared target/one build job, run
`cargo +1.96.0 test --offline --locked --release --features tool-json --lib cd_
-- --nocapture --test-threads=1`. Text captures are baseline/candidate diagnostics.
`candidate.patch` records the exact production-source and test-module wiring
relative to base, including cfg(test) counters. The oracle module itself is a
separate tracked file. Raw `identity.commit` is pre-integration base HEAD;
actual per-source hashes, not that HEAD alone, identify both tested trees.

Default and tool-json/debug-runtime full release suites, strict all-target
Clippy, fmt and diff checks pass. Tests use Rust 1.96; installed Clippy/fmt use
Rust 1.93.1. `verification.json` preserves the first verification round,
including one Python contract failure caused by the expected case list missing
the two additions. That list was updated without relaxing its exact/default
assertions; the passing rerun is in `followup-verification.json`. No runtime or
formal adapter source changed after measurement. Temporary fixed binaries and
their sidecar identities are removed after review; raw retains both identities.

The next highest-value unresolved question is which preparation component
actually dominates representative real Host request sources and their loop/wait
mix. This local result does not justify another representation or architecture
change. In particular, CB larger-loop value, compact fusion and amortized Ready
remain unmeasured; no follow-up campaign is started by this card.
