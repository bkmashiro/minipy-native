# Focused C evidence review

Target: signed clean integrated commit `e96083f2b0c0f28876d4c1b6e4ec11e171a8c30e`.
Returned tool model: `gpt-6-luna`, provider reported by reviewer: `openai-codex`.
This was one bounded read-only review, not a repeat of the core compiler audit.
No campaign was rerun by the reviewer and no files or Kanban state were changed.

Scope: cold-arrival scheduled/queue/service timing, Monty explicit-list-clear
control, memory measurement interpretation, and key tables in performance-v2.md
against the formal raw/summary. Actual measured code remains `065c7cb`.

## Confirmed finding and correction

Medium: `tools/summarize_bench_v2.py:114` retained an obsolete generated limitation
attributing Linux peak RSS to whole-process getrusage. Current Linux sampling
uses cumulative max(smaps RSS, reported VmHWM) and retains the kernel reading;
the full performance report already described it accurately.

Parent corrected the generated limitation, added assertions to the existing
Python test and regenerated the formal Linux summary on the same guest Python
3.14 plus the macOS summary on host Python 3.9. All non-limitation numeric content
is exactly unchanged when compared after normalizing list order. An initial host
regeneration of Linux floats differed by less than 2e-10 bytes in a few fitted
coefficients (Python summation version); that output was not retained. Raw timing
and memory observations were never modified.

Verification after correction: Python 6 tests pass, formal 234/234 accounting
and all 1,008 latency / 5,472 request receipts pass independent recomputation.
No benchmark/native/runtime code changed to address the finding. No other
confirmed issue was reported within the review's bounded scope. No further
review loop or benchmark campaign was requested.
