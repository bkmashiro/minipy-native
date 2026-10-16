# CW focused independent review

Target: `6717fd506ac318d1790763d7355764f971994e8a`, relative to `b21d0bf`.

One native Hermes delegate (`deleg_c44514bf`) performed the requested read-only review. Tool-reported actual model: `gpt-6-luna`; reviewer-reported provider: `openai-codex`. Verdict: approve, no blocking findings. No model substitution, builds, tests, new performance sampling or repository edits.

The reviewer checked:

- Borrowed checked conversions preserve scalar literal admission, exact 2^63 unary-negative special handling and rejection of larger magnitudes (`src/frontend.rs:1295,1350`).
- New dual-backend tests cover endpoints, bases, oversized literals, exact diagnostics, runtime overflow/short-circuit and effect suppression (`tests/cw_integer_literals.rs:10-90`).
- Fresh-source lifecycle timing includes compilation, actual execution, assertion and task teardown; allocation diagnostics are independent and requested bytes are not described as RSS (`examples/cw_probe.rs:29-112`).
- Independent in-memory raw reduction confirms 2304 measured speed records and 18 allocation records per arm. Ratios and removed-allocation counts/bytes match the committed summary. Preserved contaminated evidence and the absence of a stable broad speed claim are appropriate.

Controller acceptance: retain the two conversion replacements unchanged, stop sampling, and leave the original card for operator review before releasing CX. The post-review commit adds only this review record and its experiments link.
