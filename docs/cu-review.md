# CU focused independent review

Target: `d35de790b8c1b54946b3d4e340308281717c0b1a`, relative to integrated CS/CT baseline `c5c0295`.

Actual successful delegate model returned by the tool: `gpt-6-luna`; reviewer reports provider `openai-codex`, matching the configured delegate route. Native Hermes delegate `deleg_451a8e4a`, one task, completed synchronously in 7 API calls. Approve, no blocking findings or required modifications. This was the only independent review for CU. No builds, tests, new performance runs, repository writes or card transitions were performed by the reviewer.

Verified by the independent reviewer:

- No production `src/`, Cargo.toml or Cargo.lock change. PGO remains explicit. The recorded source, toolchain, target, flags and profile identities match the frozen plan; formal binaries are uninstrumented.
- Training excludes the original five holdouts. Only the two frozen weights were used. Warmups actually compile, execute and release fresh sources outside measured samples. Measured requests retain source compilation and release. The adapter asserts stdout/status and expected overflow/permission behavior.
- Independent in-memory reduction of all six committed compressed formal raws exactly matches `summary.json`: 528 processes, 6,336 requests. Plan collector hash matches `collector-at-start.py.txt`. The limitation on later collector hashes is documented.
- Resident tape large-count regression across both weights supports stopping default promotion. Early drift is retained. Native favorable direction does not establish production weights or a general speedup.
- Diagnostic builds are separate; phase medians are not mixed with formal totals. Execution records retain both weighted default/tool-json test pairs, successful CLI smoke and the failed initial CLI target invocation.
- Eight valid CPU intervals overlap formal activity; coarse monitoring and warmup do not prove stationarity or absence of short interference.

Additional evidence limitation highlighted by the reviewer: raw request rows retain timings, not per-request stdout/status. Successful test-process exit shows the in-process assertions passed, but outputs cannot be independently replayed from raw timings alone. The fixed fixture source and all assertions remain committed.

Controller acceptance: preserve the opt-in pipeline, no-PGO default, CS and CT. End this optimization round without extra sampling or follow-up optimization cards. Post-review changes are documentation only. Same-card operator review still controls task completion; this independent approval does not close the card.
