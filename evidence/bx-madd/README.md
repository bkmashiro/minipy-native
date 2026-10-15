# BX: bounded checked multiply-add tape fusion

Decision: **reject and remove the production candidate**. Keep the small patch,
regressions and evidence. Do not expand this into an optimizer or a tape redesign.
Base production source: `889b1348d084259734473ca17fc3ef4f9e45bda7`.
Host: macOS 26.4.1 ARM64, Rust 1.96.0. No dependency or public API changes.

## Candidate and safety contract

`candidate.patch` applies to the committed tape with its test-only diagnostic.
A local lowering peephole folds the immediately preceding `Mul(Temp, a, b)`
consumed by `Add` into `MulAdd(dest, a, b, c)`. Operands may be constants, locals,
row fields or already computed temporaries. Both `a*b+c` and `c+a*b` are eligible.
This is not tied to `total`, field offsets, Quote kernels, or a known script.

All existing recursive operand lowering runs in source order before matching.
A fallible RHS after a left multiplication emits another instruction, preventing
fusion across that error. The immediate product temporary has one consumer.
The fused handler performs checked multiplication, checks its error, then checked
addition, then one destination write. It never reassociates integer arithmetic or
writes an aliased local early. It cannot cross branch targets, Tool/await, Scan,
Next or yield boundaries. These boundaries, row fuel, cancellation, authority,
stdout limits and owner release remain unchanged. Scratch numbering is not
compacted. No analysis pass, secondary instruction storage or waiting state added.

New tests check signed extrema (including a product overflow whose unbounded
sum would fit), addition overflow, destination aliasing, computed operands,
short-circuit skipping, exact stdout prefixes and no subsequent Tool calls.
JSON type failure versus overflow provides distinguishable left-to-right errors.
The full existing suites additionally cover chunk/yield, budget, cancellation,
provider errors and lifetime behavior. Candidate passed full default and
`tool-json,debug-runtime` release suites. An initial regression draft tried to
assign a host input and was rejected by the immutable-input gate; the test now
copies inputs into mutable locals. No production rule was weakened.

## Separate layout and preparation diagnosis

`baseline-diagnostic.json` and `candidate-diagnostic.json` contain real ignored
release-libtest output. Formal binaries do not include this diagnostic test.
The prepared-byte metric is logical owned capacity, excluding allocator rounding
and Rc headers. Values use `tool-json` in both arms:

| metric | baseline | candidate |
|---|---:|---:|
| Op stride, every instruction | 56 B | 64 B |
| unmatched short: ops / capacity / prepared bytes | 4 / 4 / 376 | 4 / 4 / 408 |
| matched short: ops / capacity / prepared bytes | 5 / 8 / 608 | 4 / 4 / 408 |
| computed-operand loop: ops / capacity / prepared bytes | 11 / 16 / 1104 | 10 / 16 / 1224 |
| InterpreterUnit | 112 B | 112 B |
| scalar State | 312 B | 312 B |
| synchronous scratch array | 4096 B | 4096 B |

One instruction and one temporary write/read disappear per match, but the enum
widens by 14.3% even for unmatched code. The short script crosses a Vec capacity
threshold and shrinks; the loop's retained prepared capacity grows by 120 B.
There is no new per-task retained state. Future size, allocations and RSS were
not measured; no waiting-state density improvement is claimed.

Preparation-only diagnostic runs each perform 9 batches of 1000 preparations per
case, including drop, with no execution. Raw preserves every batch. Standalone
baseline/candidate runs have visibly different warmup trajectories (candidate
unmatched batches decline from 13.60 to 4.74 us/request; baseline stays near
3.59 us). They are not interleaved A/B and cannot establish causal preparation
regression or improvement. In particular, do not subtract these numbers from
formal E2E timings to infer execution. Formal full-request controls below include
all preparation and dispatch costs, including the unmatched Add match attempt.

## Fixed-binary resident fresh-source requests

Both arms use identical extended BT adapters. Optional `madd_short` performs
`x=x*3+x`; `madd_loop` adds `(price+1)*(quantity+1)` over 256 imported rows.
The existing Quote case is another matched path (`total+price*quantity`), scalar
and ready JSON are unmatched controls, and actual native compilation/execution
is an unchanged-production control. Output/status are asserted on every request.
These are synthetic mechanism cases, not a Monty baseline or production traces.
Default case selection is unchanged.

Each request prepares/compiles a changed source, executes and checks it, then
releases output/code in one timer. Source construction is outside; runtime,
registered Tool environment and Host fixture setup are resident, reported
separately as startup. The Quote producer constructs/imports data inside the
request. No allocation counters or compiler/interpreter profilers are active.
The driver holds the common fcntl build/run lock and checks fixed executable
hashes before/after. Source identities now recursively include Rust submodules,
fixing the previous workbench omission of `src/interpreter/tape.rs`.

Each raw archive contains two ABBA rounds over five cases and two backends:
80 successful subprocesses, 3840 measured requests (48/process). Setup and sample
0 are excluded. A/A uses the exact baseline executable in both arms. These are
ratios of arm medians of per-process request medians, not pooled samples:

| backend | case | A request ns | B request ns | A/B B:A | A/A B:A |
|---|---|---:|---:|---:|---:|
| tape | scalar | 2291.75 | 2260.50 | 0.9864 | 0.9739 |
| tape | madd_short | 4604.00 | 4458.25 | 0.9683 | 1.0109 |
| tape | Quote | 12083.25 | 11427.25 | 0.9457 | 1.0225 |
| tape | madd_loop | 14635.25 | 17249.75 | 1.1786 | 1.0165 |
| tape | ready | 11249.75 | 10781.25 | 0.9584 | 1.0412 |
| native | scalar | 44125.25 | 43364.75 | 0.9828 | 0.9971 |
| native | madd_short | 51500.25 | 52302.25 | 1.0156 | 1.0058 |
| native | Quote | 112323.00 | 112447.75 | 1.0011 | 1.0232 |
| native | madd_loop | 130978.75 | 125719.25 | 0.9598 | 1.0047 |
| native | ready | 84375.25 | 82822.75 | 0.9816 | 1.0116 |

The candidate removes dispatch but does not show a consistent net benefit across
matched requests: the computed loop is slower while the Quote loop is faster.
Small short/control deltas coexist with A/A drift up to 4.1% and unchanged native
control movement. This bounded experiment cannot attribute the loop slowdown to
a particular compiler/layout effect, nor establish a general speedup. Combined
with the certain global instruction widening, it is not worth retaining or
starting a larger campaign. Production Op/lowering/dispatch are restored exactly.
Only test-only diagnostic code, regressions and workbench/evidence changes remain.

## Reproduce

Use the existing mounted external `target` symlink; no new target or worktree.
The workbench build/run and diagnostic helper serialize on
`~/.hermes/cache/scratch/minipy-perf-build.lock`, with `CARGO_BUILD_JOBS=1`.
Use fresh evidence filenames (the tools refuse to overwrite raw files):

```
S="$HOME/.hermes/cache/scratch"
python3 tools/perf_workbench.py build --output "$S/bx-a"
python3 tools/bx_probe.py evidence/bx-madd/baseline-diagnostic-new.json
git apply evidence/bx-madd/candidate.patch
python3 tools/perf_workbench.py build --output "$S/bx-b"
python3 tools/bx_probe.py evidence/bx-madd/candidate-diagnostic-new.json
python3 tools/perf_workbench.py run --baseline "$S/bx-a" --candidate "$S/bx-b" --cases scalar madd_short quote madd_loop ready --rounds 2 --reps 48 --output evidence/bx-madd/ab-new.jsonl.gz
python3 tools/perf_workbench.py run --baseline "$S/bx-a" --aa --cases scalar madd_short quote madd_loop ready --rounds 2 --reps 48 --output evidence/bx-madd/aa-new.jsonl.gz
git apply -R evidence/bx-madd/candidate.patch
python3 tools/bx_madd_reduce.py
```

The final reducer reads the original committed evidence. For new raw use
`tools/perf_workbench.py reduce <new-raw>`. Run Rust verification under the same
lock and shared target with `cargo +1.96.0 test --offline --locked --release`,
then `--features tool-json,debug-runtime`. `cargo fmt` uses the default installed
rustfmt; Rust 1.96.0 does not have that component installed on this host.
Fixed executables are temporary and removed after verification. Raw manifests
preserve hashes, host/toolchain, source identities and process details.
No Linux/Wasm execution or Monty performance claim is made.
