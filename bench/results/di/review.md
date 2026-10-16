# Focused read-only review

Fixed implementation: c65004174774684016fc663e64ddb472acce0f05
Tree: f377834e46b416f5c2c7995c4edcd9a2e761b5f6
Parent: 757cb0c

Delegate tool result reports actual model `gpt-6-luna`, 4 API calls, exit_reason
`completed`, duration 41.78 seconds. Delegation provider configuration was read
as `openai-codex` before dispatch; the child environment also reported it. The
child correctly did not claim it could independently verify session metadata.
The parent verified the returned result model, rather than trusting that claim.

Scope: examples/di_corpus.rs, tools/di_corpus.py, tests/di_corpus.rs,
tools/di_corpus_test.py, fixtures/di/smoke.jsonl, docs/di-corpus.md,
bench/results/di/, and the relevant existing Provider/authorization call sites.
No private DH data, Cargo rerun, file edit or Kanban transition by the child.

Commands included git status --short --branch, git show --no-ext-diff --stat
--oneline --decorate=short c65004174774684016fc663e64ddb472acce0f05, and
git rev-parse c65004174774684016fc663e64ddb472acce0f05^{tree}; targeted source
reads/searches are present in the delegate tool trace.

Result: no confirmed blocker in whole-source admission/no replay, controlled
submission mismatch handling, per-request setup/teardown timing, separation of
ownership diagnostic from timing, or unknown total-task-memory boundaries.
JSONL matching was checked against the submitted v1 documentation/implementation
and synthetic fixture, not against the independent private DH artifact.

Implementation remains unchanged after this review. This record and the
experiments update are documentation-only followup. Public synthetic smoke is
not a private corpus campaign, a correctness pilot or a performance conclusion.
