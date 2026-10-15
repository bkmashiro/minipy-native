# Focused independent BZ review

Fixed reviewed commit: `24518d5` (base `f311832`).
Actual delegated model verified by execution receipt: `gpt-6-luna`.
Verdict: no blockers or actionable findings in the bounded read-only review.

Reviewer confirmed final production source/manifests match the base; the
candidate stays archived. They reviewed both capture call sites, shared helper,
validation before authorization, continuation liveness, transient budget fallback,
Box deallocation before marker, cancelled detached parameter lifetime and the
formal adapter interval. They independently ran `tools/bz_detach_reduce.py` and
reproduced both complete 64-process / 2048-request archives, waiting-payload
observations and narrow defer/no-stable-latency-win conclusion. BY's per-Ready
yield is explicitly distinguished from an untested amortized allowance design.

The early failed admission test and outdated Python case-list verification remain
preserved beside corrected passing verification. Reviewer flagged this only as a
record-reading note; README already identifies the corrected files. They did not
run Cargo/builds/new benchmarks, modify files, commit, push or close the parent.
Lead ran full corrected candidate and final default/tool-json-debug-runtime
release suites and final strict Clippy, rustfmt and nine Python tests.

A preceding attempt to run the exact requested model through standalone Codex CLI
failed with 'gpt-6-luna is not supported ... with a ChatGPT account'. It performed
no review and was not counted. The Hermes delegate above actually used the
requested model; there was no silent substitution.

No Linux/Wasm/Monty performance, allocator census, RSS or application wait
prevalence was established. This following documentation commit changes no
implementation or measurement.
