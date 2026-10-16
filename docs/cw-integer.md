# CW: checked scalar integer literal conversion

## Change and invariant

Base: `b21d0bf`. The only production change is in `src/frontend.rs`: convert borrowed RustPython `BigInt` using `i64::try_from`, and recognize the magnitude of the unary-negative minimum with `u64::try_from(n) == Ok(1_u64 << 63)`. No dependency, authorization, runtime or JSON-path change. The cached locked `num-bigint` 0.4.8 implements `TryFrom<&BigInt>` by checked `ToPrimitive`, without creating an owned BigInt or decimal string (`src/bigint/convert.rs:107-140`).

The existing bool arm, out-of-range error kind/reason, expression error-location wrapper and checked runtime unary subtraction are unchanged. Negative minimum still bypasses positive-i64 admission only for exactly 2^63. Larger magnitudes still fail admission, while negating the admitted negative minimum again fails at execution. The f-string formatter still needs decimal output and is deliberately unchanged; this experiment removes scalar conversion round trips, not string formatting.

## Bounded experiment

Commands from repository root:

```sh
python3 tools/cw_run.py A  # on base production frontend, new probe present
# apply the two production replacements
python3 tools/cw_run.py B
python3 tools/cw_measure.py
python3 tools/cw_measure.py --reduce  # read raw only, no timing/build
```

Build wrapper holds `fcntl` lock `/Users/yuzhe/.hermes/cache/scratch/minipy-perf-build.lock`, uses existing `target` symlink to `/Volumes/M.2/caches/minipy-native-target`, Rust 1.96.0, one job, offline/locked release. No new target/worktree/download. Speed uses `tool-json`; separate allocation build uses `json-allocation-profile`. Requested allocation bytes count allocator Layout sizes, not RSS or live retained bytes. Allocation-build times are not performance evidence.

`examples/cw_probe.rs` runs real tape/native compilation and execution in a resident current-thread Tokio host. Each request makes a distinct source via a numbered comment, prepares, executes, checks exact stdout/status and drops output/source/unit inside timing. Host runtime and empty TestProvider are resident outside timing. No source cache, warm-unit reuse, effects or provider waits. Eight warmup requests per process are excluded. Cases are no integer (`print(True)`), ordinary three-integer short arithmetic and 32 repeated large negative literal assignments. These are synthetic compiler controls, not production coverage or a Monty comparison. Probe was rustfmt-formatted between A and B without behavioral changes.

`bench/results/cw/*_identity.json` records production frontend hash, base HEAD, rustc identity, build commands and saved binary hashes. Both arm HEADs are base because production diff was not yet committed. Saved binaries remain only in ignored existing target, not committed. Raw CSV, per-launch top-CPU snapshots and reducer summaries are committed.

First ABBA + A/A had XprotectService at 76.4% CPU at entry and severe no-integer drift. Preserve it as `contended_{speed.csv,load.json,summary.json}` and do not use it for improvement claims. After observing Xprotect absent from top CPU and no competing build, performed exactly one bounded replacement run. Entry snapshot: controlling worker 11.5%, VM 5.3%, proxy 3.5%; `load.json` records every launch. No claim of perfect system isolation. Each run has 36 processes and 2304 measured requests (6 cells, ABBA plus A/A, 64 requests/process), plus 288 warmups. Allocation diagnostics are separate 12 processes and 36 measured requests total.

An initial Python CSV buffer placed the allocation header after subprocess rows. Fixed runner to flush header and moved that header to the beginning of the two captured allocation files without changing observations. The failed initial reduction did not trigger a measurement retry.

## Results

Ratios use mean of per-process medians: (B1+B2)/(A0+A3). A/A is second/first process median. This is intentionally a small check, not a confidence interval.

| backend/case | B/A | A/A | removed allocation calls | removed requested bytes/request |
|---|---:|---:|---:|---:|
| tape/no integer | 1.0313 | 1.1168 | 0 | 0 |
| tape/short | 1.0021 | 0.9833 | 6 | 28 |
| tape/dense | 0.9388 | 1.0056 | 128 | 2432 |
| native/no integer | 0.8802 | 0.9741 | 0 | 0 |
| native/short | 1.0296 | 1.0013 | 6 | 28 |
| native/dense | 0.9955 | 1.0115 | 128 | 2432 |

Retain the tiny change because it removes measured unnecessary allocation/work and simplifies the conversion. Dense tape has a favorable signal. Ordinary short scripts and native do not establish a stable speed benefit; even the integer-free native control moves substantially. Do not generalize these ratios to application performance. Stop sampling.

## Correctness

Lock-held actual commands:

```sh
CARGO_TARGET_DIR=$PWD/target CARGO_BUILD_JOBS=1 RUSTUP_AUTO_INSTALL=0 cargo +1.96.0 test --offline --locked --test cw_integer_literals --test scripts --test interpreter --test boundaries --test builtins_admission
CARGO_TARGET_DIR=$PWD/target CARGO_BUILD_JOBS=1 RUSTUP_AUTO_INSTALL=0 cargo +1.96.0 test --offline --locked --features tool-json --test cw_integer_literals --test string_tool
```

43 default tests passed (8 boundaries + 4 builtins + 1 CW + 16 interpreter + 14 scripts). Six tool-json tests passed; two explicitly ignored existing performance tests stayed ignored. New test checks zero/negative-zero, ordinary positive/negative, both i64 endpoints, hex/octal/binary/underscores, huge magnitudes and one-past limits, exact admission kind/reason/line/column parity, runtime double-negation overflow, prior stdout and later-effect suppression, and short-circuit behavior on both real backends. Existing suites protect bool typing, budgets, cancellation, owner lifetime and Tool ordering. First CW test compile mistakenly used nonexistent `CompileError.message`; corrected to actual `reason` plus exact kind/location after reading the definition. Rust 1.96.0 rustfmt is unavailable; existing default rustfmt 1.8.0 formatted only new Rust files. No components installed. Linux validation was not performed for this frontend-only change.
