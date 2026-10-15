# CC: accept bounded direct-argument waiting-memory policy

Base: main `0a23fac`. CB remains isolated on `exp/cb-copy-patch` (`814fdd8`).
Decision: integrate the BZ mechanism as a private waiting-payload optimization.
No public setting, new dependency, wait predictor, scheduler or backend is added.
The changed acceptance criterion values the demonstrated waiting-memory benefit;
it does not reinterpret BZ's mixed timing as a speedup.

## Policy and tradeoff

Both engines call one synchronous helper at direct JSON string Tool capture:

- Run existing string validation/decode and scan-fuel charging first.
- Copy only an unescaped validated leaf whose root has no continuation consumer.
- Require logical root length >= 65536 B and leaf length <= 4096 B.
- Require existing transient available bytes to cover the leaf. This includes
  live roots, decoded-string capacities and external Quote payloads. Insufficient
  headroom returns the original owner, introducing no new task error.
- Preserve authorization/call debit and the existing post-capture root release.
  No await, speculative operation poll or change to detached operation ownership.

Cutoffs are private engineering bounds, not measured workload optima. At the
eligibility boundary the payload ratio is at least 16:1. CPU work adds one bounded
allocation/copy, at most 4096 B per eligible capture, with no new scan/fuel debit.
Peak incremental payload is at most 4096 B while the original root still exists,
and must fit the existing budget. Allocator rounding, control blocks, heap metadata
and preexisting operation overhead are outside this logical byte budget/census.
Permission failure can discard the bounded copy before launch, in the same argument
validation-before-authorization order. No effect is moved earlier.

Small/live roots add only fixed eligibility checks after the existing capture.
Escaped strings already own decoded text, joined strings already materialize and
remain outside this helper. A sliced/shared Bytes may keep invisible backing or
other external owners: length is not capacity, and this policy cannot guarantee
physical deallocation for every provider. It can independently own the small leaf
without retaining the argument's root clone. It adds no future/frame/state fields.

Accept this small policy because freeing a large otherwise-dead waiting owner is
valuable even without a request speedup. Reuse BZ timing evidence (not rerun):
detach B:A tape 1.01754/native 1.00914, with control/process drift, no stable causal
latency benefit or regression. Workload prevalence, real wait distribution and
process RSS benefit remain unmeasured. No claim that every workload benefits.

## One gated concurrent experiment

`tests/cc_waiting.rs` runs 12 cohorts per arm: six modes on tape and native,
eight guests/owned operations per cohort, 96 tasks total. Every operation must
signal entry and remain blocked on a Notify gate before census. Compilation and
fixture construction are deliberately outside the observation; this is a payload
lifetime census, not a request benchmark. The shared unit does not hold payloads.
A source fixture string exists separately and is excluded from the owner census.
Each delivered source owner is its own exact Box<[u8]> in Bytes::from_owner.
Drop explicitly deallocates the Box before incrementing the release marker.
No Weak references or external clones retain these delivered backings.

| mode, per backend (8 concurrent) | baseline argument payload/capacity B | final B | final source backing deallocated B |
|---|---:|---:|---:|
| dead root, 10 B leaf | 8388968 | 80 | 8388968 |
| live root | 8388968 | 8388968 | 0 |
| exact-root budget | 8388968 | 8388968 | 0 |
| 4097 B leaf | 8421664 | 8421664 | 0 |
| small root | 1384 | 1384 | 0 |
| escaped leaf | 80 | 80 | 8389008 |

Each mode aliases a nested JSON path. After census half the guests are cancelled
while all operations stay blocked: cancelled guests return Cancelled, and uncopied
backings remain held by the operations. Then the gate opens, every operation reads
the exact expected argument, including cancelled guests, and drops it. Remaining
guests succeed with exact stdout (including the live-root continuation). All
source owners are eventually deallocated. A 30 s timeout guards test hangs only;
no wait duration or throughput number is inferred. Identical results on both engines.

The baseline was run before production changes; `CC_CANDIDATE=0` selects baseline
assertions only, not another runtime path. Final ordinary tests expect the accepted
policy without environment variables. Baseline source hashes capture its version
of the test; the only later test difference is the candidate-default/comment.
`candidate.json` records the first passing integration; `final.json` adds exact
cutoff/headroom/scan-fuel unit coverage. These are verification repeats of the same
bounded fixture, not independent performance samples.

## Validation and reproduction

macOS 26.4.1 ARM64. Rust tests: 1.96.0, offline/locked release, one build job.
Installed Clippy/rustfmt: 1.93.1, no component download. External target symlink
verified at `/Volumes/M.2/caches/minipy-native-target`; existing fcntl exclusive
`~/.hermes/cache/scratch/minipy-perf-build.lock` covers all builds/census runs.
No worktree, new target, dependency download, CI or temporary executable copy.

Full default and tool-json/debug-runtime release suites pass, including imported
BZ root/cancellation regression. Strict all-target Clippy, rustfmt and diff check
pass. Unit coverage checks exact 64 KiB/4096 B cutoffs, shortfall by one byte,
external Quote accounting, live root suppression and unchanged scan debit.
Existing suites retain authority/error order, resource/effect, real HTTP permit
and detached late-completion coverage. No Linux/Wasm run or Monty comparison.

```
# On base, with the same test present; does not alter production:
python3 tools/cc_verify.py baseline evidence/cc-waiting/new-baseline.json
# On accepted source (all required checks):
python3 tools/cc_verify.py final evidence/cc-waiting/new-final.json
# Replay committed census and assert complete cohort keys/counts:
python3 tools/cc_waiting_reduce.py
```

Fresh output names are mandatory. To replay the baseline without checking out a
new worktree, temporarily reverse only the production diff in the three runtime
files (not the unit test) under a clean, saved checkout, then restore it before
final tests. Raw includes source hashes, host/toolchain, commands and exit status.
No raw timing campaign or RSS measurement is included.

## Stage conclusion

- BV: defer schema-validation API and architectural changes, no proven worthwhile
  request saving. Operation/guest single-job topology and single-PC native entry
  remain untested here.
- BW: keep synchronous sink borrowing, one escaped-join allocation eliminated on
  both engines, no stable E2E speed claim.
- BX: reject the measured generic multiply-add representation: global instruction
  widening and mixed requests. Alternative compact fusion remains untested.
- BY: reject explicit Ready's measured representation (future +216 B, mixed E2E).
  It yielded after every Ready; amortized-allowance continuation was not tested.
- BZ/CC: accept bounded waiting-owner detachment for memory benefit, without a
  latency or RSS claim. Real workload prevalence remains unknown.
- CB: real clang templates and direct installation work, but the optimistic raw
  install/call/free floor (~3.989 us) already consumes the tiny full tape request
  (~3.542 us). Stop only this tiny-glue/per-request JIT-module design. Full source
  backend and loop-size matrix were not built; larger-loop value is untested.
  See the preserved experiment branch, not an assertion that all C&P is futile.

Stop this stage. No new experiment or optimization is implicitly scheduled.
