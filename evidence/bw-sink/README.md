# BW: synchronous string sink borrowing

Decision: keep the internal work elimination, without claiming an E2E speedup.
Base: `c8d28f07ed9fbeb26444c308642767f34d7d56bc`. macOS ARM64,
Rust 1.96.0. No public API, async Tool ownership, task state or dependency change.

The synchronous joined/f-string print path previously constructed owned
`StringValue`s for every piece, cloning literal/JSON owners and promoting a
cached escaped `String` to `Arc<String>`. It now validates and sizes each piece
in the same order, storing only formatted scalars. After authority, total output
budget and reservation succeed, it borrows the prepared immutable text and
commits the line. There is no await, install or release between those passes.
The existing plain JSON print already borrowed and is a control. Async Tool
arguments still acquire ownership using the original path.

## Separate allocation/layout diagnosis

`baseline-diagnostic.json` and `candidate-diagnostic.json` contain real release
libtest output, not formal latency samples. Current-thread allocator counting is
active only around execution plus output/code release, after a warm execution.
Compilation and runtime setup are outside this diagnostic. Both backends print
the same escaped text with either `print(text)` or `print(text + '')`.

- Baseline: plain 15, joined 16 allocations, tape and native.
- Candidate: plain 15, joined 15 allocations, tape and native.
- The regression intentionally fails on the baseline. For that baseline check,
  only `append_string_source` was temporarily restored to its exact old body;
  the cache-accessor extraction and new tests remained. Neither adds allocation.
- Output is 72 bytes, SinkPart 56 bytes, its 32 temporary slots 1,792 bytes in
  both checks. No persistent structure/async fields changed. The existing JSON
  State/StringCache tests still assert 128/24 bytes. Future size, RSS and retained
  waiting-state density were not measured; no density improvement is claimed.

Reproduce under the common fcntl lock, with
`CARGO_TARGET_DIR=/Users/yuzhe/projects/minipy-native/target`,
`CARGO_BUILD_JOBS=1`, using:

```
cargo +1.96.0 test --offline --locked --release --features tool-json --test tool_validation_cost synchronous_join -- --nocapture
cargo +1.96.0 test --offline --locked --release --features tool-json --lib borrowed_sink -- --nocapture
```

The second test checks malformed later text before print permission, scan-budget
failure before permission, denied print and atomic stdout-budget failure. Both
backends' integration tests cover UTF-8, escaped strings, repeated joined pieces,
f-strings with signed extrema/bools, multiple JSON owners, Tool captures and
cancelled detached operations. All default and tool-json release tests pass,
as does strict all-target tool-json Clippy, rustfmt and diff check.

## Formal resident fresh-source requests

Both fixed binaries use the identical extended BT test adapter. Each request
prepares/compiles a fresh source with the resident registered Tool environment,
executes it, checks exact status/stdout, then releases output/code before the
single timer stops. Runtime, registration and HTTP fixture setup are separately
reported as startup, never included in request samples. Caller source construction
is outside the interval. No allocation counters or compiler phase timers are
active in the formal binary. The fixture is synthetic ready Host JSON, not a
real filesystem or Monty baseline. Scalar and Quote are unchanged-path controls.

Each raw archive has two ABBA rounds, 112 subprocesses and 5,376 measured requests
(48 per process); sample 0 and setup are excluded. All processes and assertions
succeeded. A/A runs the exact A executable in both arms. Ratios below are the
ratio of arm medians of process request medians, not pooled samples:

| backend | case | A/B B:A | A/A B:A |
|---|---|---:|---:|
| tape | scalar | 0.9999 | 0.9951 |
| tape | Quote | 1.0176 | 0.9871 |
| tape | plain JSON | 1.0000 | 1.0236 |
| tape | escaped JSON | 0.9663 | 1.0076 |
| tape | plain join | 0.9813 | 1.0042 |
| tape | mixed f-string | 0.9695 | 0.9916 |
| tape | escaped join | 0.9730 | 0.9961 |
| native | scalar | 0.9821 | 0.9889 |
| native | Quote | 1.0139 | 1.0036 |
| native | plain JSON | 1.0087 | 1.0005 |
| native | escaped JSON | 1.0226 | 1.0039 |
| native | plain join | 0.9942 | 0.9983 |
| native | mixed f-string | 1.0119 | 0.9879 |
| native | escaped join | 0.9935 | 1.0003 |

The A/B run has large early process drift, including unaffected tape controls.
These small mixed deltas do not establish a stable full-request gain or regression.
Keep the measured allocation removal and eliminated reference-count traffic;
do not promote microdeltas into performance claims or start another campaign.

## Reproduction and identity

Use `tools/perf_workbench.py build` to save A on the base production paths with
the current adapter, then B with `candidate.patch` (the patch also includes the
private atomicity/layout test). Hold the driver-managed common build/run lock,
use the mounted external shared target and no extra worktree/target. Example:

```
python3 tools/perf_workbench.py build --output /Users/yuzhe/.hermes/cache/scratch/bw-a
python3 tools/perf_workbench.py build --output /Users/yuzhe/.hermes/cache/scratch/bw-b
python3 tools/perf_workbench.py run --baseline /Users/yuzhe/.hermes/cache/scratch/bw-a --candidate /Users/yuzhe/.hermes/cache/scratch/bw-b --cases scalar quote plain_sink escaped_sink joined_sink fstring_sink escaped_join --rounds 2 --reps 48 --output evidence/bw-sink/ab.jsonl.gz
python3 tools/perf_workbench.py run --baseline /Users/yuzhe/.hermes/cache/scratch/bw-a --aa --cases scalar quote plain_sink escaped_sink joined_sink fstring_sink escaped_join --rounds 2 --reps 48 --output evidence/bw-sink/aa.jsonl.gz
python3 tools/bw_sink_reduce.py
```

The archive manifests retain binary hashes, per-source hashes, toolchain/build
commands, host, base commit and tracked-diff identity. Full per-process summaries
are reproduced with `tools/perf_workbench.py reduce <raw>`. Temporary fixed binaries
are removed after the run; shared target remains. Source preparation, output and
checks remain identical in both arms. No Linux/Wasm or Monty performance claim.

Next bounded opportunity belongs to the pre-created BX card: generic checked
multiply-add tape fusion. No further string/ownership architecture change here.
