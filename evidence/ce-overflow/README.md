# CE: pure native arithmetic overflow exits

Decision: keep one bounded native-only candidate. Actual control work and code
size fall, and two matched synthetic fresh-source requests have lower medians
than A and their exact-binary A/A controls. This is local supporting evidence,
not a general speedup claim. Do not extend the campaign to chase drift.
Base `e3055b2f85bd5872eb3aee1eb2ea0dd45e8e1ff2`; macOS ARM64, Rust 1.96.0.
CC/CD and isolated CB branch `814fdd8` remain intact.

## Contract and implementation

`TaskError::Overflow` has no position or operation identity. Native arithmetic
exits all return status 1. Within one pure arithmetic subtree, allow only i64
literals, bound scalar locals, validated row fields/buffer lengths, and checked
add/sub/mul. Trees with at least two arithmetic operations share an overflow
exit. Emit the same tree left-to-right, retain every intermediate overflow bit,
OR those bits, and branch before returning the expression value to its consumer.
No reassociation or only-final-result overflow test is used.

After an intermediate overflow, remaining pure operations may run with wrapped
values. They cannot trap or produce observable effects. Field/length loads use
already validated, live leases at the current valid scan row. Even if subsequent
arithmetic returns in range or multiplies by zero, a prior flag is still set.
Assignment, print, comparison, Tool capture/dispatch and all later statements
remain behind the error branch. Short-circuit RHS selection is unchanged. JSON,
abs/min/max and other non-allowlisted nodes use the existing lowering; a pure
child subtree may optimize without crossing its surrounding boundary. Existing
JSON-vs-overflow error ordering remains distinct and tested.

The small local classifier adds a traversal of eligible trees, plus bounded
failed classification work for mixed trees (existing frontend depth bound 32).
This cost is included in fresh preparation. There is no CFG framework, persistent
metadata, allocation, tape/Op/ABI/runtime change, check removal, resource-policy
change, new dependency or backend. Default checks still enforce every overflow.

## Actual lowering diagnostic, separate from formal requests

`baseline-code.txt` and `candidate-code.txt` preserve actual pre-backend CLIF and
post-register-allocation Cranelift AArch64 VCode, not handwritten assembly.
`CE_BYTES` is the compiled machine-code size reported by Cranelift. The input-
bound four-operation short tree has 4 overflow `brif` / native `cbnz` exits in A
and 1 in B; loop arithmetic has the same 4-to-1 change. Both keep all four flag
producers. B adds three ORs and removes three conditional exits (and associated
lowered branches/blocks); size is 228 -> 212 B short, 600 -> 528 B loop.
Cranelift did not already remove the baseline control work.

The diagnostic short source uses bound inputs, unlike the formal BT tiny source
`x=n; x=x*3+x; print(x)`, whose literals permit backend constant folding. Do not
attribute the tiny fresh-request difference to four runtime branch eliminations.
The computed-operand diagnostic loop is the BT `madd_loop` expression. Neither
size is a task-memory, allocation, RSS or density measurement. Test-only VCode
printing/disassembly is absent from formal integration-test binaries.

## Bounded full-request measurement

Existing BT driver and adapters are unchanged. Each A/B and A/A archive has two
ABBA rounds, 80 successful processes and 3,840 measured requests. No failed,
missing, status or stdout observations. Per-process setup and sample zero are
excluded. Each request prepares fresh source, executes/checks status and stdout,
and releases code/output before its timer stops. Caller source construction,
Host registration, runtime and loopback fixture startup remain outside the named
request boundary. Quote generation/import is inside each request. These are
synthetic Ready Host fixtures with 256-row loops, not real external workloads.
No phase clocks or allocator diagnostics are enabled in formal binaries.

Ratio of arm medians of per-process medians, B:A:

| backend | case | A/B | A/A |
|---|---|---:|---:|
| native | tiny multiply-add | 0.94332 | 0.99696 |
| native | computed-operand loop | 0.94984 | 0.99806 |
| native | original Quote loop | 0.99565 | 0.98357 |
| native | scalar control | 0.98413 | 1.00414 |
| native | Ready JSON control | 0.96048 | 0.98970 |
| tape | tiny multiply-add | 1.01433 | 1.02164 |
| tape | computed-operand loop | 1.01318 | 1.00784 |
| tape | original Quote loop | 0.98087 | 0.99634 |
| tape | scalar control | 1.03321 | 1.01440 |
| tape | Ready JSON control | 1.00613 | 0.99799 |

Matched native tiny medians: 47.958 -> 45.240 us; computed loop:
127.312 -> 120.927 us. Controls also drift, particularly native Ready (-4.0%),
and one of four candidate tiny processes is slower than all four baseline tiny
processes. Thus these data do not establish a precise causal 5% speedup or a
statistical no-regression bound. The keep decision combines certain work/code
reduction and the bounded matched-request evidence, with unchanged layout and
modest local implementation cost. Original Quote shows no established benefit.
No throughput, long-loop, Linux, Wasm or Monty performance claim.

## Reproduction, identity and verification

External `/Volumes/M.2` was mounted, `target` resolved to
`/Volumes/M.2/caches/minipy-native-target`. All builds/measurements used the
common fcntl lock `/Users/yuzhe/.hermes/cache/scratch/minipy-perf-build.lock`,
shared `CARGO_TARGET_DIR=/Users/yuzhe/projects/minipy-native/target`, one build
job and `cargo +1.96.0 --offline --locked` for builds/tests.
No new target, worktree, dependency, CI or website was created.

Environment deviation: running `cargo +1.93.1 fmt` based on the prior documented
installed-toolchain label caused rustup to automatically download/install that
named toolchain. Existing default `stable` already reported Rust 1.93.1. This
violated the card's no-download restriction; it is not described as compliant.
No default toolchain/config/global kernel change was made. Subsequent commands
used installed stable fmt/Clippy and `RUSTUP_AUTO_INSTALL=0`.

`baseline.patch` and `candidate.patch` reconstruct the exact measured codegen
sources on base; all other manifest source hashes equal base blobs. The reducer
verifies every source identity in A/B and exact-binary A/A, complete counts and
all measurements, then emits `summary.json`:

```
python3 tools/ce_overflow_reduce.py
```

To rebuild arms, use each patch on base with the identical existing BT adapter:

```
python3 tools/perf_workbench.py build --output /Users/yuzhe/.hermes/cache/scratch/ce-a
python3 tools/perf_workbench.py build --output /Users/yuzhe/.hermes/cache/scratch/ce-b
python3 tools/perf_workbench.py run --baseline /Users/yuzhe/.hermes/cache/scratch/ce-a --candidate /Users/yuzhe/.hermes/cache/scratch/ce-b --cases scalar ready madd_short quote madd_loop --rounds 2 --reps 48 --output evidence/ce-overflow/ab.jsonl.gz
python3 tools/perf_workbench.py run --baseline /Users/yuzhe/.hermes/cache/scratch/ce-a --aa --cases scalar ready madd_short quote madd_loop --rounds 2 --reps 48 --output evidence/ce-overflow/aa.jsonl.gz
```

Build/run driver locks itself and refuses existing output paths. For diagnostic
and verification cargo commands, take the same fcntl lock and shared target/job
environment. Run `cargo +1.96.0 test --offline --locked --release --features
tool-json --lib ce_arithmetic_code -- --ignored --nocapture --test-threads=1`.

Candidate full default and tool-json/debug-runtime release suites pass. New
native/tape regressions include intermediate add/sub/mul overflow followed by
in-range cancellation/zero, stdout prefix, self-assignment and short-circuit.
JSON tests distinguish which error happens first and prevent later publishing.
Existing budget, permission, cancellation, owner and CC/CD regressions also run.
Final source differs from measured candidate only by rustfmt and Clippy's
semantically equivalent collapsed `if let` condition; final CLIF, VCode and
sizes exactly match the candidate diagnostic. Final suites, strict all-target
Clippy, fmt, nine BT Python tests and diff checks pass, recorded in
`final-verification.json`. `clippy.txt` preserves the first collapsible-if
failure; it was fixed without suppression. The reducer's initial newline
reconstruction assertion failed before evidence generation and was corrected;
all measured source hashes now verify. No timing reruns were used to replace
failed or noisy observations. Temporary fixed binaries are removed after review.
