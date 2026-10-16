# CT focused independent review

Target: `a5efb032349e19f8a0acba04fb5054973ccf79e8`, relative to CS `96a59548eed8f0f128437c24e72be21c1fed281e`.

Actual successful delegate model returned by the tool: `gpt-6-luna`; reviewer reports provider `openai-codex`. Native Hermes delegate `deleg_14d44a0a`, one task, completed synchronously in 8 API calls. Approve, no blocking findings or required modifications. Review was read-only: no builds, tests, new performance samples or repository writes.

The initial standalone Codex CLI route rejected `gpt-6-luna` with HTTP 400 before review. It did not produce a review and is not counted as an independent audit. The successful native delegate used the same requested model; no model substitution occurred.

## Verified by the independent reviewer

- Numeric JSON leaves remain I64; typed leaves retain type and order. Schema references are preparation-only. `Plan::path` owns bound keys, and the finished Program owns its Plan. No environment lifetime is added to runtime state.
- Fourteen recorded structured admission errors match A/B exactly, including priority, full reason and source line/column. Lifetime regressions cover prepared tape/native execution after Environment destruction for both schema variants.
- Optional compiler-profile timers are nested as documented and excluded without that feature. Success and failed-phase tests cover all three native Tool entry points. The reviewer inspected these tests and their committed logs, rather than executing them.
- Independent in-memory reduction of compressed raw exactly matches committed summary: 192 processes, 9216 requests, 24 cells and 48 measured samples per process. Binary/source/adapter identities match. Formal A has no production diff; formal B changes only frontend schema borrowing.
- The first small tape cell's 2.249 B/A and later 0.996 A/A are preserved. Wide tape results are a local synthetic signal. Three sparse CPU blocks with no guard rejection do not exclude short interference or explain process drift. No broad performance/no-regression conclusion is justified.

Controller acceptance: retain the measured copy-removal cleanup and the separate optional diagnostic correction. Stop the matrix as prespecified. Integration after this review changes only documentation/evidence; no source changes are needed. Same-card operator review still controls completion and release of the existing CU dependency.
