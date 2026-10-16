# CN: bounded Rust-host LLVM PGO

Decision: keep the reproducible experiment, defer enabling production PGO. The
unchanged release configuration remains the downstream baseline. There is a
promising held-out native-request signal, but this small synthetic experiment
does not establish production workload weighting or justify distributing a
fixture-trained profile. Stop without more samples or tuning. No main integration
is performed by this card.

## Identity and what is optimized

Base: `907eb40efa01cce148924000ddb965878ee8e38b`, Apple M4/macOS 26.4.1,
Rust `1.96.0 (ac68faa20 2026-05-25)`, LLVM `22.1.2`. Every `src/**/*.rs`,
`Cargo.toml`, `Cargo.lock`, and `tests/perf_workbench.rs` hash is identical across
A/instrumented/B and matches the base Git blobs. No production source, admission,
checked arithmetic, authorization, resource/cancellation, owner, error order or
public API changes. The adapter is the existing BT adapter, not a replacement
business kernel.

Rust's LLVM compiles MiniPy's parser, lowering, tape interpreter, runtime,
Cranelift library code and Rust dependencies. PGO optimizes that ahead-of-time
Rust executable. User-source native code is still compiled by the existing
Cranelift JIT. This is not an LLVM user-source backend and does not replace the
native/tape comparison with a hard-coded answer. Prebuilt Rust std/runtime
components are not rebuilt with instrumentation.

Existing release configuration is only `[profile.release] debug = 1`; defaults
for optimization/LTO/codegen units/CPU remain unchanged. Both arms use the same
explicit `--target aarch64-apple-darwin`, so host build scripts/proc macros are
not affected by target-only RUSTFLAGS. The only candidate options are
`-Cprofile-use=<absolute merged.profdata>` and diagnostic
`-Cllvm-args=-pgo-warn-missing-function`. Instrumented training uses only
`-Cprofile-generate=<absolute profiles directory>`. Final timing uses neither
profile-generate nor compiler-profile instrumentation.

## Tool preflight and costs

Apple's installed `llvm-profdata` reports LLVM21, incompatible with Rust LLVM22.
No matching tool was initially present in the Rust sysroot. The authorized small
official component was installed using:

    RUSTUP_AUTO_INSTALL=0 rustup component add llvm-tools-preview --toolchain 1.96.0

Official Rust 1.96 manifest selects
`https://static.rust-lang.org/dist/2026-05-28/llvm-tools-1.96.0-aarch64-apple-darwin.tar.xz`,
SHA256 `e6ab00bed593386d2b074bed46715407377e8a686a67a5d9b2fe351f93684a62`.
HTTP Content-Length is **47,464,228 B**. The installed component's 14 files occupy
**423,769,624 logical / 423,800,832 allocated B**. This is the llvm-tools component,
not a full LLVM distribution. No toolchain/default override was changed. Matching
profdata reports `LLVM version 22.1.2-rust-1.96.0-stable`; its hash is recorded in
`profile.json`. The component remains available for reproduction.

Before building, external disk had 293 GiB free and shared target `du -sh` was
23G. After all builds/tests, target occupies **26,710,272 KiB** (`du -sh` 25G).
The pre-run size was rounded, so no exact net disk increment is claimed.
737 distinct existing files declared in the three Cargo JSON builds total
**1,259,104,872 logical / 1,260,683,264 allocated B**. This includes shared existing
artifacts and omits unreported object and full-test outputs; it is not a net-delta
estimator. `inspection.json` preserves these scopes and commands' outputs.
Scratch binary/profile material occupies **75,300 KiB**. No binary, target tree,
profraw or profdata is committed. Shared build artifacts are not blanket-cleaned.

| Release test executable | File bytes | Build wall seconds |
| --- | ---: | ---: |
| A | 10,725,216 | 119.388 |
| instrumented, training only | 20,160,320 | 112.117 |
| B, profile-use | 11,138,480 | 111.459 |

B is **+413,264 B / +3.85%** versus A. These are the same BT integration-test
executable, including libtest/adapter and debug=1, not a shipping service's size.
Build times have different cache conditions, not evidence of faster compilation.
Actual Mach-O `__TEXT` is A/B **7,749,632/7,831,552 B**. `llvm-size`'s `others`/total
include Mach-O virtual regions such as `__PAGEZERO`; do not treat its ~4 GiB total
as physical file size or RSS. Defined `__llvm_profile` symbols exist in training
only, absent in both formal binaries (`inspection.json`).

## Frozen training and held-out requests

Reuse the external target symlink
`target -> /Volumes/M.2/caches/minipy-native-target`, one Cargo build job,
`~/.hermes/cache/scratch/minipy-perf-build.lock`, offline/locked Rust1.96.
No new Cargo target root, worktree, dependency, CI or alternate platform. The
explicit target triple creates/reuses Cargo's subdirectory inside that same root.

One instrumented build, ten training processes, 25 fresh sources each:

- tape/native × scalar, original Quote weighted scan, ready JSON/contains,
  plain string sink, real pooled loopback HTTP.
- These 250 training requests, including each initial request, are not formal
  speed samples. Producer construction, source preparation and execution are
  actual work, not traces synthesized from a timing model.
- Ten profraw files total **30,438,736 B**. Matching profdata merge exits0, producing
  **4,607,072 B**, SHA256
  `aae1bb6b2b42bcd1bf76160aa192df0228998ae9805aa7e7d60ea38933993d0a`.
- Profile inspection finds 16,610 functions, 175,238 blocks and total count
  22,670,549; these are profile inventory/counts, not coverage percentages.

Holdout programs are disjoint from the training case list: branch,
scalar_inputs, computed-operand madd_loop, escaped_join, Notify-gated detach_wait.
They cover scalar/branch, an alternate 256-row checked scan, escaped JSON string
composition and ~1 MiB producer allocation plus real Tool suspension. They share
BT's Host/declarations and language families with training. This reduces literal
fixture overfit, but is not an independently sampled production corpus.

One ABBA round per backend/case, 12 measured requests plus one warmup per process.
A/B and exact-A A/A each have **40 successful processes / 480 measured requests**,
no task/status/stdout or wait-stage failures. Both binaries use the same sources
and registered entrypoint. All full request intervals include prepare, execute,
producer construction/import, code/output release and wait-controller cleanup.
Caller source/expected string construction, reporting and resident fixture startup
are outside, as in [BT](../../docs/perf-workbench.md).

Fixture startup is reported separately: pooled median setup A/B campaign
**70,375.5 ns**, A/A **71,625 ns**. These aggregate setup medians are not startup
speed comparisons. Process launch/dynamic linking are outside both intervals.
There are no per-phase diagnostic timers in formal timing.

## Descriptive result, no precision claim

Median of two process medians per arm. Ratios below are B/A; lower is faster.
All samples, including early drift, are retained. No additional round or case
selection after viewing results.

| Holdout | Tape A µs | Tape B µs | Tape B/A | Tape A/A | Native A µs | Native B µs | Native B/A | Native A/A |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| branch | 23.521 | 13.948 | .59300 | .93207 | 79.969 | 61.490 | .76892 | 1.00504 |
| scalar_inputs | 7.750 | 6.500 | .83871 | .95131 | 56.927 | 45.782 | .80421 | .99804 |
| madd_loop | 26.239 | 21.375 | .81461 | 1.04364 | 126.750 | 92.062 | .72633 | 1.03174 |
| escaped_join | 19.771 | 16.927 | .85615 | 1.04903 | 86.104 | 61.375 | .71280 | .99154 |
| detach_wait | 86.479 | 85.740 | .99145 | 1.00383 | 157.906 | 133.490 | .84537 | 1.00688 |

Native's five positive directions, especially escaped_join/madd_loop/detach_wait,
are larger than this separate A/A's movements and are useful local support for
Rust-host PGO. They are not precise causal 15–29% production gains. Tape's first
branch A0/B1/B2/A3 process medians are **37.271/20.167/7.729/9.771 µs**: large
process drift invalidates a causal 40.7% tape claim. Tape wait shows no clear
benefit. Small independent process counts, cache/CPU drift, common Host adapter
and unknown real-source frequencies remain limitations. A/A is not a noise bound.
No PMU/cache, RSS, density, throughput, Linux/Wasm/Monty or unchanged Code Mode
coverage claim; CG's workload-fit stop remains in force.

Profile-use emits **913** missing-function warnings in the formal BT build.
Different default and debug-enabled full-test artifacts emit 9,937/34,949 such
warnings, including untrained test monomorphizations and cold code. They are
recorded, not suppressed. Merge succeeds; these warnings are not proof of profile
coverage or production robustness. No training expansion after seeing warnings.

## Correctness, records and reproduction

PGO-built complete release default suite: **74 passed / 0 failed / 3 ignored**.
PGO-built `tool-json,debug-runtime`: **132 passed / 0 failed / 11 ignored**.
Both include doc tests; ignored benchmarks/diagnostics are not counted as passed.
Build/test command wall times and outputs are preserved in `execution.json.gz`.
Nine BT Python contract tests, py_compile, stable fmt and Git diff checks pass.
`src/`, `tests/`, Cargo manifest/lock remain byte-identical to the base.

Files:

- `ab.jsonl.gz`, `aa.jsonl.gz`: complete fixed-binary manifests, per-process rows,
  status, stdout oracle success and end records.
- `*-identity.json`: full source/adapter/flags/toolchain/binary identities.
- `profile.json`: actual merged profile and component-tool hash, training/holdout
  case order, individual profraw hashes, not the profile payload.
- `inspection.json`: symbol-level no-instrumentation check, actual size and disk
  inventory. No new performance run.
- `execution.json.gz`: actual successful commands, Cargo compiler-message warnings,
  full suite output, training request rows and profdata inspection.
- `summary.json`: read-only reducer output; all process medians retained.

Executed:

    python3 tools/cn_pgo.py collect
    python3 tools/cn_pgo.py inspect
    python3 tools/cn_pgo_reduce.py
    python3 -m unittest discover -s tools -p test_perf_workbench.py -v
    python3 -m py_compile tools/cn_pgo.py tools/cn_pgo_reduce.py
    git diff --exit-code 907eb40 -- src tests Cargo.toml Cargo.lock
    git diff --check

Read-only replay uses only the reducer and committed data; it validates all source
hashes against base Git blobs and both complete campaign manifests. Compare its
parsed JSON with `summary.json`. Collection/inspection refuse overwrite. A new
experiment requires archiving this evidence and this experiment's own
`~/.hermes/cache/scratch/cn-pgo` directory first; do not delete the shared target.
No rerun is needed for this decision. Reproduction needs the matching official
component already installed or explicitly installed as above.

Collector-only provenance note: after the actual collection, a logging-expression
typing fix and explicit selected-environment logging were added, plus the separate
`inspect` action. The original execution rows do not contain the added environment
field. Training backend/case identity is reconstructed from the frozen nested
loop order recorded in `profile.json` and the ten successful request subprocesses;
it is not independently echoed in each training row. No measured Rust source,
adapter, training split, build flags or sample schedule changed, and no timings
were rerun. These limitations are explicit rather than backfilling observed env.

Next lane must use unchanged release baseline, not these scratch PGO artifacts.
Do not conflate any later kernel result with this build experiment. Mainline
integration remains subject to original-card review.
