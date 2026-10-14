# D current optimization pointer

D follows C (`d42e0cd`). Default top-level scripts now compile to the same semantic
IR/native CFG as the old benchmark entry. No implicit expression echo. Tool calls
remain sequential and truly asynchronous; scalar print uses bounded private output.

## Kept, with matched evidence

- Cleared Cranelift compiler scratch reuse: Linux 100-row fresh-script wall median
  390.23 -> 338.77 us. Retained weighted-script scratch is about 33 KiB per compiling
  thread. M4 improvement is near noise; timer tails varied.
- Live scalar writeback at native suspension: Linux 100-row 356.89 -> 331.73 us;
  10k 443.91 -> 410.79 us. Weighted machine code 648 -> 572 bytes on Linux.
- Bounded geometric stdout growth: 4,096 scalar prints, Linux cold 467.74 ->
  378.84 us, prepared 223.09 -> 148.13 us. Reallocation calls 4,095 -> 12.
  Final capacity grows from 9,103 to 16,384 bytes in that workload, within its cap.
- Separately boxed Quote payload: Linux 1M-row cold 5.479 -> 4.900 ms, prepared
  5.182 -> 4.501 ms; requested peak about 32.05 -> 16.05 MB. Explicit Weak control
  confirms payload release at record wait. QuoteWeak now aliases Weak<Box<[Quote]>>.

The operator's latest unattended authorization explicitly covers measured narrow
engineering/API changes. No owner approval gate remains. Legacy compile/execute
signatures are unchanged; observers and TaskError::Input are documented migrations.

## Rejected and stop point

ISA/module preparation was only 1.31 us in the initial Linux profile, versus
299.82 us backend compilation. No ISA cache was built. opt_level=none was unstable
and enlarged code. Incremental owner accounting was also unstable and regressed
the 10k prepared cell, so both were reverted.

The current dominant small-script cost is backend compilation. Integrated scalar
cold wall is 190.00 us native vs 56.63 us ordinary Monty; 100-row printing script
640.99 vs 247.92 us. At 10k rows it is 466.03 vs 3,799.23 us. VM process rounds
vary considerably; these are bounded synthetic observations, not service capacity.
Generalization has a measured cold cost: old-source same-round C/D totals grow
380.33 -> 525.02 us at 100 rows, while prepared totals fall 8.67 -> 6.73 us.

Stop new experiments here. No remaining simple candidate has evidence of net
value. Broader compiler/admission changes need another focused cost hypothesis.
First-class bytes/string values still require encoding, ownership and Tool-ABI
choices; private stdout does not implement that language path. No source cache,
verifier removal, scheduler rewrite, framework or unrelated project change.

## Handoff

45 core tests pass on macOS and actual Linux ARM64; bench 12, Python 6. Original
cancellation, late completion, overflow and budget tests remain intact. The three
real script paths and renamed/branched/two-loop/loop-local cases are covered.
Frame is 320 bytes versus 280 previously; each declared buffer adds a 16-byte view
and an 8-byte owner slot. Printing after record requires retaining code until done.

Raw results, commands, identities and recomputation live in
`bench/results/d-script/` and [experiments.md](experiments.md). The one final
independent review of `255e4eb` found no confirmed blocker. Actual reviewer identity
`gpt-6-luna/openai-codex` is verified by CLI usage metadata; 45 core and benchmark
tests were independently rerun. See `focused-review.txt` and its identity JSON.
Native delegation's two-child budget was exhausted by frontend/correction lanes,
so the review used a separate read-only Hermes CLI session without changing
profiles, budgets or configuration. No further optimization lane is scheduled.

## E diagnostic follow-up

The explicitly assigned E lane is complete as a diagnosis, not a new optimization.
[compiler-profile.md](compiler-profile.md) contains the six-shape, same-machine
Linux profile and its reproducible raw evidence. D's runtime/semantics are unchanged.
Backend define occupies 82.1--85.8% of diagnostic compile wall; register allocation
and five enabled verifier calls lead the exclusive buckets. ISA/module preparation
and finalize/load remain small. IR grows from 17 instructions/6 blocks for empty
source to 168/41 for two scans and conditional Tool continuations.

There is no backend-replacement case yet. Recommended first bounded experiment:
predicate-oriented CFG lowering, then no-suspension entry specialization. The
single-pass allocator is a lower-priority conditional retest, not a new first
choice. Historical `564381d` old-IR/M4 ablation (10k weighted rows/chunk4096,
ready providers, four rounds) with diagnostics omitted and repeated verification
retained regressed source-cold 117.604 -> 134.7705 us and prepared
20.2295 -> 42.792 us. Evidence: `/Users/yuzhe/.hermes/cache/scratch/minipy-hotspots/FINDINGS.md:3,10,14--21,33`.
Those are old source-to-result observations, not current Linux compile times.
The current general CFG/Linux combination has not been retested; its larger
regalloc bucket does not predict a net gain or cancel the known runtime risk.
Any authorized retest must retain all checks and qualify cold source-to-result,
prepared runtime and code size. E implemented or scheduled none of these
candidates. Profiling adds material overhead and
process variation, so pass costs are diagnostic observations, not plain-release
times. New experiments still require a focused replacement contract and evidence.

## F bounded predicate follow-up

F keeps the explicitly authorized predicate-only lowering: weighted/nested
blocks 25 -> 23 / 39 -> 31, bytes 572 -> 548 / 704 -> 636. Value consumers,
checks, effects and suspension remain unchanged. The single noise follow-up on
identical Linux binaries gives cold 10k source-to-result 324.437 -> 287.923 us
weighted and 466.784 -> 414.577 us nested, with unchanged scalar cold control
130.797 -> 130.881 us. All three cold rounds agree; prepared nested has a +6.7%
first round retained beside two improved rounds, so no blanket execution claim.
Mac default/diagnostic and actual Linux each pass 48 tests. Full boundaries,
initial noisy evidence, the one follow-up and keep decision are appended to
[compiler-profile.md](compiler-profile.md#f-predicate-lowering-experiment-2026-10-14).
F stops at focused same-card review, without implementing another E candidate.
