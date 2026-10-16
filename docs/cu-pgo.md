# CU: optional Rust-host PGO on the integrated baseline

Decision: keep PGO as an explicit experimental build, do not enable it by default.
The fixed formal matrix completed without replacement samples. Both training
weights regress the large-count tape request even after 64 fresh-source warmups.
Ordinary `cargo build --release` remains unchanged.
No production source, language, authorization or lifecycle change is part of CU.
Baseline is c5c0295, including CS's scalar count kernel and CT's schema borrowing.
The benchmark adapter adds explicit fresh-source warmup and three composition
oracles. It does not replace user-source compilation with a handwritten kernel.

## Frozen experiment

The original CN builder and CR macOS contention assessment are reused from
44212e0 and c58cea8 respectively. CN's build helper additionally accepts the
existing diagnostic feature; old collection entry points retain their historical
paths. Use `tools/cu_pgo.py`, not CN/CR's old campaign commands, for this tree.
No old profile is used.

Rust 1.96.0, LLVM 22.1.2, target aarch64-apple-darwin, tool-json, default release
optimization/LTO/CPU settings. Matching llvm-profdata is already installed in the
Rust sysroot. All builds and runs serialize through the existing fcntl lock;
one Cargo job and the existing mounted external target are required. No target
root, worktree, dependency or global toolchain setting is added.

Training: tape and native each run scalar, quote, ready, plain_sink, loopback,
count_price_small. Balanced: 24 measured requests plus one initialization request
per cell. Fixed string-heavy distribution: ready/plain_sink/loopback each 96,
others 24. These are synthetic weighting choices, not observed workload weights.
Both distributions use the same instrumented executable, independent process
profiles and independent merges. Original five holdouts and new compositions
are excluded from both training lists. Each source has a different numeric literal.

Formal cells: branch, scalar_inputs, madd_loop, escaped_join, detach_wait,
multi_branch, multi_overflow, multi_permission, quote, ready,
count_quantity_large, on tape/native. The first five match CN's held-out families.
Three independent compositions combine sequential Tools, branches, partial stdout
and runtime overflow/permission errors. They still share the same Host and language
families, and are not an independently sampled production corpus. Quote, ready
and the large count control are explicitly not family-independent holdouts.

For each of 1 and 64 fresh-source warmup requests per process, run exactly one
ABBA round per distribution and one exact-A A/A. There are 12 measured requests
per process; the retained sample 0 is the last warmup, never measured. Earlier
warmups actually compile/execute/release, but emit no rows. A process reuses its
resident runtime/Host/HTTP pool, not prepared user code. This is 528 formal
processes and 6,336 measured requests if the full matrix completes. A/A is a
separate drift diagnostic, not a confidence bound. No extra round after inspection.

Preflight requires two non-boot-average top intervals with >=90% idle and no
unrelated process at >=50% of a core. During collection: >=85% idle and the same
competitor threshold; one violating completed interval stops only our benchmark
process group. This coarse 1-second monitor can miss short contention. It cannot
prove thermal/frequency/cache stability. Preserve failures, do not replace samples.

## Cost and diagnostic boundaries

Formal binaries have neither profile-generate nor compiler-profile enabled.
Their full interval includes preparation, producer/import, execution, code/output
release and wait-controller cleanup. Resident fixture/runtime construction and
source/expected-output construction are outside, as in BT. Setup is separately
recorded. Process launch/dynamic linking are outside both intervals.

Separate compiler-profile builds use the corrected registered-entry timing on
the original holdout cases after 64 warmups. Record parse/type/frontend, setup,
lowering, backend (`define_function`), finalize and cleanup (`CodeUnit` release),
along with execution-including-code-release. These diagnostic builds/profile
monomorphizations differ from formal builds; diagnostic phase medians must not
be added to formal totals or presented as exact formal shares. Training weights
are never adjusted after seeing missing-profile warnings or diagnostics.

## Commands and evidence

    python3 tools/cu_pgo.py collect
    python3 tools/cu_pgo.py measure
    python3 tools/cu_pgo.py diagnose
    python3 tools/cu_pgo.py summarize
    python3 -m unittest discover -s tools -p test_perf_workbench.py -v

Optional application build, only after collection produced matching profiles:

    python3 tools/cu_pgo.py build --distribution balanced

This verifies source/adapter/profile hashes and the exact Rust version, then runs
an explicit target/features/profile-use Cargo build under the same lock. It copies
the `minipy` CLI into `~/.hermes/cache/scratch/cu-pgo/minipy-balanced`, records its
identity and executes the existing weighted_script fixture. `--distribution strings`
selects the other frozen profile; it is not an automatic best-profile selector.
The test executable supplies training for shared Rust-host code, not all CLI/test
monomorphizations. Missing-function warnings remain visible in command records.
Uninstrumented ordinary builds need no profiles and remain the default:

    cargo +1.96.0 build --offline --locked --release

No RUSTFLAGS export or persistent Cargo configuration is made by these commands.
The optional CLI smoke establishes functionality, not CLI performance improvement.

Collection refuses an existing evidence/scratch directory. Each subsequent stage
checks the frozen source identity. Run stages once and preserve incomplete data.
Read-only summary replay requires only the committed compressed formal rows and
BT reducer, not the uncommitted executable/profile payloads.

`evidence/cu-pgo/plan.json` freezes weights/sources/adapter/schedule before building.
`*-identity.json`, `profiles.json`, `machines.json` and `execution.json.gz` retain
actual flags, toolchain, source and binary hashes, profile hashes/inventory,
binary sizes, symbol-level instrumentation checks and build/test commands/output.
`collection.json.gz` retains monitor samples and campaign interval overlap.
`*-w*.jsonl.gz` preserve every formal request, per-process identity and completeness.
`diagnostic-*.json` are explicitly separate from the formal matrix.

Binary trees, profraw and profdata remain in the existing external target and
private scratch; none are committed. Profile identity is tied to source, adapter,
toolchain, target, features and weights. Rebuild/retrain after those change rather
than assuming an old profile transfers. Source schema borrowing is retained
regardless of PGO results. No Linux/Wasm/Monty, throughput or density claim follows
from this macOS bounded experiment.

## Fixed formal result

All six campaigns complete: 528 processes / 6,336 measured requests, with matching
stdout/status oracles, including expected Overflow/Permission and retained earlier
stdout. Balanced training executed 300 requests, string-heavy 732, including their
initialization requests. No training measurement enters the table. No samples
were dropped and no further campaign was run.

After 64 fresh-source warmups, median of two process medians per arm; B/A <1 is
faster. A/A is separately collected with the exact same A binary.

| Case | Tape balanced | Tape strings | Tape A/A | Native balanced | Native strings | Native A/A |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| branch | .8164 | .8734 | 1.0016 | .7527 | .7515 | 1.0008 |
| scalar_inputs | .8000 | .8086 | 1.0306 | .7464 | .7533 | 1.0031 |
| madd_loop | .8222 | .8083 | .9814 | .6961 | .7130 | .9894 |
| escaped_join | .8092 | .8215 | .9836 | .7447 | .7373 | .9871 |
| detach_wait | .9842 | .9435 | .9950 | .8265 | .8402 | .9933 |
| multi_branch | .8332 | .7845 | 1.0679 | .7241 | .7115 | .9688 |
| multi_overflow | .9020 | .8264 | 1.0062 | .7258 | .7250 | .9770 |
| multi_permission | .9026 | .7936 | .9843 | .7272 | .7329 | .9796 |
| quote | .7860 | .7961 | .9641 | .7083 | .7120 | .9873 |
| ready | .8805 | .8073 | .9896 | .7307 | .7298 | .9885 |
| count_quantity_large | 1.1285 | 1.1165 | 1.0048 | .8973 | .8830 | 1.0090 |

The count request regresses on tape in every early/resident weighting comparison.
Resident balanced A/B is 214.208/241.740 us; strings 213.125/237.948 us. Both B
process medians exceed both A medians within each campaign. The resident 11.6–12.9%
regressions are materially larger than the separate count A/A movement. This is
a full-request regression, not an attributed per-kernel measurement. The fixed
stop rule rejects default promotion without another round or weight search.

Native has a consistent favorable direction across both weights and both warmup
regimes (resident B/A .6961–.8973). That supports a local Rust-host optimization
signal, including frontend/Cranelift work, but does not establish production
weights or a portable speedup. The two distributions share all training families.

Early tape results retain large drift: branch balanced/string B/A 2.4277/2.8752,
with respective B process medians 34.938/7.750 and 43.979/7.834 us. Early scalar_inputs
and madd_loop also regress in the aggregate. Do not reinterpret these as stable
2–3x causal slowdowns or discard the first B process. Formal source literals are
2–13 after one warmup and 65–76 after 64, so the warmup regimes are separate
matched A/B distributions, not a controlled estimate of warmup cost alone.
64 requests are a fixed engineering warmup, not proof of stationarity.

Eight valid completed top intervals overlap the campaigns. Minimum idle 85.12%,
maximum listed competitor 34.4% of one core; the predefined guard did not reject.
CR's reused guard does not enumerate descendants, so our Python driver and renamed
candidate processes can conservatively remain in that list. It does not establish
that all listed processes are unrelated. These sparse samples do not rule out the
observed short process drift.

CU ends this optimization round. No automatic JIT, SIMD, kernel DSL, language or
Tool follow-up campaign is authorized here. The ordinary release remains the
downstream baseline; neither PGO weighting is selected as a new default.

## Separate attribution diagnostic

The corrected native registered entry reports nonzero frontend timings. Example
compiler-profile medians in us, after 64 warmups, one process per cell:

| Case / build | Frontend | Lowering | define_function | Finalize | Code release |
| --- | ---: | ---: | ---: | ---: | ---: |
| branch / A | 7.188 | 2.000 | 55.583 | 2.167 | .542 |
| branch / balanced | 6.375 | 1.916 | 53.105 | 1.917 | .542 |
| branch / strings | 6.334 | 1.875 | 52.313 | 1.875 | .542 |
| madd_loop / A | 11.209 | 3.750 | 101.854 | 1.917 | .583 |
| madd_loop / balanced | 10.188 | 3.708 | 99.896 | 1.917 | .583 |
| madd_loop / strings | 10.292 | 3.730 | 99.313 | 1.917 | .542 |

These builds are separately compiled with compiler-profile/Cranelift timing and
have different profile-missing functions. They were not run inside the formal
CPU guard. They locate substantial native preparation work in Rust-host Cranelift
and show changes in more than one phase, but do not explain the formal gain by
adding these smaller phase differences. Tape reports parse/type timings; its
native-only fields remain zero, not zero tape preparation cost. Tape's diagnostic
branch preparation also drifts (A/balanced/strings 14.521/24.896/20.042 us), despite
the fixed warmup. The measured count regression has no phase isolation here.
No PMU or instruction-layout causal claim is made; no function tuning followed.

Provenance: `collector-at-start.py.txt` hashes exactly to the plan's collector
SHA256. The later collector adds source/profile/toolchain validation, the opt-in
CLI command and read-only diagnostic reduction. Training weights, adapter,
formal schedule and production sources are unchanged. Those later stages do not
echo their own collector hash, and this limitation is not backfilled as an
observed field. Formal manifests independently echo the unchanged BT driver hash
and all binary/source identities.

## Costs and acceptance

`costs-and-validation.json` is a small index into complete `execution.json.gz`.
Formal baseline/profile-generate/balanced/strings builds took 15.394/116.979/
113.361/113.971 s in this already-populated shared target; these are observed
incremental build costs, not clean-build comparisons. Formal binary bytes:
10,740,640 baseline, 11,134,752 balanced, 11,134,704 strings (both about +3.67%).
Each profile has 16,627 function entries and is about 4.61 MB. Entry inventory is
not execution coverage. Formal PGO builds each emit 895 missing-function warning
lines, versus 4,625 for diagnostic builds; warnings are retained rather than used
to tune training or represented as complete profile coverage.

Both distributions pass the actual default (79 tests) and tool-json (133 tests)
release suites, 424 tests total across the four commands, in the frozen toolchain,
flags, target and fcntl lock. Native/tape error, lifecycle, owner, budget and
authorization suites remain intact. Compiler-profile's new code was already
validated by CT; CU does not rerun that prior campaign. Python workbench/policy
contracts pass 10+3 tests, along with rustfmt and diff checks. The initial CPU
guard unit fixture omitted top's trailing column, then was corrected; the guard
itself was unchanged. No production source or Cargo diff from c5c0295.

The balanced opt-in CLI build and real fixture run pass: stdout 41,
`status=ok loads=1 records=1`, and final trace has zero buffer/frame bytes. Its
build is a cache hit (0.114 s) after full release tests, not a cold build. One
earlier optional invocation incorrectly used the package name as `--bin` and
failed before compilation. The corrected target is `src/bin/minipy.rs` / `minipy`;
both invocations remain in the evidence. No formal run was repeated.

Observed allocated `du -sk target/` rose from 27,313,460 to 30,453,532 KiB; this
is a shared-tree before/after observation, not an exclusive per-build allocation
measurement. Private CU scratch is 170,924 KiB and committed evidence about
4,680 KiB. No dependencies, extra targets/worktrees, large machine-code dumps,
profraw/profdata or executables are committed. These costs plus the tape regression
reinforce keeping the pipeline opt-in and ending this optimization round.

The sole focused independent `gpt-6-luna / openai-codex` review approves fixed
implementation commit d35de79, with no blocking changes. [Review record](cu-review.md).
Formal raw stores timings rather than per-request status/stdout; successful
process exit attests the committed in-process oracles passed, not a standalone
output replay from the timing rows. Operator review on the original card remains
the final task gate.
