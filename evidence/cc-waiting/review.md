# CC focused fixed-snapshot review

Reviewed commit: `29077ada853d50222913323793e0c1f5a597da28`.
Base: `0a23facd7910ac1f49e569bb4b412012dbac8e6a`.
Reviewer: actual `gpt-6-luna`, via Hermes one-shot with `openai-codex`.
Execution init receipt identifies that model and session `20261016_022835_49bc3e`;
the session returns normal exit 0. No model substitution. Lead retains board
lifecycle, acceptance, verification, signing and publication ownership.

Verdict: no confirmed blockers in this bounded independent read-only review.
Reviewer traced shared capture helper, both continuation-mask call sites, existing
root/decoded/external budget accounting, validation/scan charging before authority,
post-capture release and detached operation ownership. They checked Box drop-before-
marker semantics, all-eight gated observation, cancellation and live continuation
controls, and the deliberately limited allocation-vs-RSS claims.

They replayed `python3 tools/cc_waiting_reduce.py`: 12 cohorts / 96 tasks per arm,
both engines release 8,388,968 B source backing while retaining 80 B arguments
for dead roots. Other controls remain unchanged. They checked all 22 final raw
source hashes against reviewed commit blobs. They performed read-only git/status,
source/test/evidence inspection and hash/reducer commands, no build, benchmark,
file modification, commit, push or board action. Full Rust suites/Clippy/fmt are
lead-run evidence, not claimed as independently rerun by the reviewer.

Nonblocking provenance note: raw `identity.commit` is pre-commit HEAD (the base),
not the integration SHA. Source hashes identify the tested tree and match the
fixed review snapshot. Following documentation clarifies this, and corrects the
baseline replay instruction to reverse the full three-file source diff including
its unit test before running the focused baseline fixture. No runtime/test/raw
content changed after review.

The stage conclusion distinguishes BW allocation removal, rejected BX/BY measured
representations, untested compact fusion/amortized Ready and CB's tiny-glue,
per-request-module stop with larger-loop value untested. Real wait prevalence,
RSS, threshold optimality and latency benefits remain unestablished.
