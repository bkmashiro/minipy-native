# BV: input validation diagnosis gate

## Decision

Defer all three BU mechanisms. No candidate implementation or public InputSchema
API was introduced. This is a bounded macOS diagnostic and A/A observation, not a
negative A/B proof that validated input schemas can never help. No runtime, Tool
handoff, authority, resource, cancellation or language behavior was changed.

Two short identifiers cost a batch-amortized **84.60 ns** validation median in the
private diagnostic, versus **3.567 µs** for complete tape prepare/release. These
are independent intervals, not additive phases. The 2.4% ratio is descriptive,
not a measured removable fraction of a formal request. Validation ranges from
54.17 to 207.38 ns; full preparation ranges from 2.377 to 6.751 µs. The zero-input
check is already tiny. This does not justify a new public API in this run.

Native cold preparation remains dominated by backend work in the prior separate
BT diagnostic (78.7% median backend/prepare; see docs/perf-workbench.md). It is not
a new BV measurement or evidence that input-schema removal speeds native requests.

## Identity and procedure

Source base: `7f1da48165bc31150e4ee9038fe16cb337ca0ec3` (clean main on entry).
Apple M4, macOS 26.4.1 arm64, rustc `1.96.0 (ac68faa20 2026-05-25)`.
Each raw manifest contains source/executable hashes and exact build commands.
Only existing dependencies, one mounted external target and one build job were
used. Builds and measurements held the shared fcntl perf lock. No new target,
worktree, service, registry, scheduler or backend was created.

Formal binary SHA256:
`d3376ab7ede145e59fbe91033bc71626da774bd6bf4da6ce9fe1ecfa0aa339e0`.
The driver was subsequently adjusted only to keep the original four default
cases; the recorded run explicitly selects its cases, so its sequence is unchanged.

## Private tape preparation diagnosis

`src/frontend/schema_probe.rs` is gated by test + tool-json + non-Wasm. It calls
existing private validation, registered-environment frontend, and complete tape
preparation APIs. It never skips validation. Validation uses black_box and 4096
calls per interval; frontend and tape preparation each use 256 fresh sources per
interval. Every prepared Program/unit is dropped inside its own interval. Source
construction and empty Tool environment registration are outside. Two processes
each run eight rounds, alternating 0/2-input and phase order. All rounds, including
the first, are retained; no manually chosen warmup exclusion is applied.

| Inputs | Validate ns median [min,max] | Frontend ns median [min,max] | Tape prepare ns median [min,max] |
| --- | --- | --- | --- |
| 0 | 5.04 [3.67,17.44] | 3324.46 [2104.00,14631.51] | 3337.08 [2177.25,7869.63] |
| 2 | 84.60 [54.17,207.38] | 3407.14 [2209.47,7075.68] | 3567.22 [2377.44,6751.46] |

Statistics are over 16 per-batch averages per input count. Frontend includes source
checks, input validation, parse, typing/binding and Program destruction. Tape
preparation also includes flattening/liveness and unit destruction, but independent
intervals cannot be subtracted to isolate lowering. This experiment does not split
RustPython parse from typing/binding. No compiler-profile hooks, counting allocator,
per-request phase timers or global sampler is active. This micro-diagnostic is
separate from formal timing and is not pooled with it. Allocation savings were
not measured. A preliminary version measuring validation/full-prepare only was
run once before adding frontend timing; its console output was not archived and
none of its numbers contribute to this table.

The identical source in 0/2 diagnostic controls is `print(n + 1)` and does not use
the injected locals. Formal two-input cases below do use both inputs. Neither
synthetic fixture establishes an actual Host schema distribution. No 32-input or
long-name speed case was selected to amplify the benefit.

## Formal fixed-binary A/A

`aa.jsonl.gz`: 64 processes, two ABBA rounds, 32 measured requests plus sample 0
warmup per process, 2048 measured requests total. All processes exit 0; every
request asserts exact status/stdout. Both arms are the same formal binary AND
registered entrypoint. Host/runtime/16 Tool declarations stay resident. Setup is
reported separately. Each timed request includes new-source prepare, real tape
or native execution, ready Tool work if selected, stdout check/drop, unit/task
and machine-code release. Source and expected-output construction are outside.
No prepared-unit reuse, cache, per-phase profiler or allocator instrumentation.
The ready JSON Bytes payload is static, as in BT; this is not a remote-I/O test.

| Backend/case | A ns | B ns | A/A B/A |
| --- | ---: | ---: | ---: |
| tape scalar (0) | 2614.75 | 2333.50 | 0.8924 |
| tape scalar (2) | 3239.50 | 3333.50 | 1.0290 |
| tape ready (0) | 11198.25 | 10927.50 | 0.9758 |
| tape ready (2) | 12468.50 | 12396.00 | 0.9942 |
| native scalar (0) | 40947.75 | 40385.25 | 0.9863 |
| native scalar (2) | 50437.50 | 47052.25 | 0.9329 |
| native ready (0) | 82155.75 | 80312.50 | 0.9776 |
| native ready (2) | 91406.00 | 91427.25 | 1.0002 |

Values are medians of per-process medians, not pooled request medians. Individual
process medians/min/max are reproduced by the existing reducer. The first tape
scalar A process median is 12354.5 ns versus later A medians 2583.5/2646/2375 ns;
substantial cold/process drift remains despite sample 0 exclusion. Do not discard
that process. The two-input scalar A/A absolute median difference (94 ns) and
ready difference (72.5 ns) are the same order as the isolated validator observation.
This is not a statistical noise bound, nor a measured A/B benefit. Stop without
adding a schema API or running more samples until a favorable answer appears.

## Deferred mechanisms and invariant ledger

- Input-schema token: legally could hoist immutable validation, while keeping
  mutable Compiler inputs/bindings and runtime value checks. Current small/empty
  input evidence does not establish request-level value; defer before adding API.
  This does not revive the withdrawn HashSet-to-pairwise experiment.
- Operation/guest continuation single-job service: defer. Removing spawn/oneshot
  requires a separate ownership/driver design, not a JoinHandle replacement.
  Must retain submitted effects and arguments after cancellation, drop unrelated
  guest owners/code promptly, poll detached operations to completion/timeout,
  keep permits until actual completion, and preserve Tool-boundary fairness.
  Rc native code is not Send. New completion/cancel state and provider future
  storage might cost more than the deleted handoff. No fresh evidence justifies
  that architecture here; BE/BF/AL negative results still apply.
- Single-PC native entry: defer. A tiny no-suspension lowering shortcut does not
  address tape costs or real Tool waits; no new profile justifies reopening the
  old PC candidate. No fixture-specific Rust kernel or auto-tiering is added.

Since production paths did not change, code/owner lifetime, effects, permissions,
input checks/error ordering, stdout, budget, fairness and future layout follow
identical code. Future size, retained wait bytes, RSS and allocation counts were
not newly measured; no density improvement is claimed. HTTP permit/cancellation
and large-operation future experiments were not run because their candidate was
deferred. No Monty, Linux, Wasm or real application performance conclusion.

## Reproduction and checks

Use fresh output names; tools refuse to overwrite raw. Scratch executable is
removed after the final handoff. From the repository:

```sh
S="$HOME/.hermes/cache/scratch"
python3 tools/bv_schema_diagnose.py evidence/bv-input-gate/schema-diagnostic-new.jsonl.gz
python3 tools/perf_workbench.py build --output "$S/bv-formal-new"
python3 tools/perf_workbench.py run --baseline "$S/bv-formal-new" --aa \
  --cases scalar scalar_inputs ready ready_inputs --rounds 2 --reps 32 \
  --output evidence/bv-input-gate/aa-new.jsonl.gz
python3 tools/perf_workbench.py reduce evidence/bv-input-gate/aa.jsonl.gz
python3 -m unittest discover -s tools -p test_perf_workbench.py -v
```

Schema reduction: parse gzip JSONL, require two successful processes and end
record; flatten their 32 rows; divide validate_batch_ns by validate_reps and
frontend_batch_ns/prepare_batch_ns by prepare_reps; report median/min/max per
input count, retaining all rows. Raw includes full subprocess stdout/stderr.
Focused release correctness passed: default input-schema/interpreter/scripts
29 tests (1 existing timing test ignored); tool-json library/input-schema/
ToolEnvironment/string/HTTP 37 tests (5 diagnostic/timing tests ignored).
Strict tool-json Clippy over all tests, rustfmt check and 9 Python reducer tests
passed. Commands used cargo +1.96.0 with --offline --locked --release and the
shared lock/target/jobs=1 settings above. Independent fixed-commit review is
recorded in the task handoff. No full campaign rerun or new production optimization
is kept.
