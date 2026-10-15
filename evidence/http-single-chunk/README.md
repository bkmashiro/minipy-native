# HTTP singleton collector experiment

## Decision

Revert the production candidate. A complete release ABBA confirmation produced baseline run medians 66,209 / 66,125 ns and candidate 76,042 / 56,333 ns. The median of run medians is 66,167 vs 66,187.5 ns: effectively unchanged amid large variation. This does not establish a speed benefit. The candidate removes one singleton container allocation (128 requested bytes in the isolated allocation diagnostic), but increases the concrete fetch future from 592 to 632 bytes and starts a multi-chunk Vec at capacity 2 instead of the baseline 4. Multi-chunk speed was not measured.

The original production collector is restored; only a test-only future-size probe remains in `http_provider.rs`. The complete candidate patch is preserved.

## Measurement

Base commit aa836c66555e073d33651519e99aea9c1fcbc96e, macOS ARM64, Rust 1.96.0, `tool-json`, release. The confirmation's frontend matches the base. All source switching, compilation and measurement was serialized by the shared `minipy-perf-build.lock`, using the existing target directory and one build job.

`HttpProvider::call_tool` calls a real local TCP server through pooled Hyper, receiving a 256-byte Content-Length body. Each process measures 500 sequential calls with one permit. Source preparation is excluded. Raw samples and summaries are printed after timing. Results include the first request, with no warmup exclusion. Assertions check 500 responses, zero payload copies and returned permit. This is a transport-boundary experiment, not a full MiniPy request benchmark.

- `release-baab/`: complete confirmation (the directory name is historical; actual order is **baseline/candidate/candidate/baseline**, ABBA). Four logs contain 2,000 raw request timings, source hashes and release future-size outputs. `build-identity.txt` records compiler and command.
- Root `run-1.txt.gz` / `run-2.txt.gz`: earlier incomplete baseline/candidate attempt, 1,000 samples. Source switching incorrectly assumed alternating arms and failed on the second consecutive candidate. These observations are not pooled with the confirmation. Frontend was still the unrelated input-schema candidate for those two HTTP runs, which do not execute it.
- Earlier debug summaries are discarded as performance evidence.
- `candidate.patch`: collector and test-only probe delta against the base.
- `tests/http_collect_cost.rs`: real transport measurement and isolated allocation mechanism diagnostic.

## Reproduction

The runner now handles consecutive identical arms, refuses an existing output directory, and restores source in `finally`. The complete confirmation exercised these fixes.

```sh
python3 evidence/http-single-chunk/campaign.py /path/to/new-evidence-directory
python3 evidence/http-single-chunk/reduce.py evidence/http-single-chunk/release-baab
```

The reducer verifies every sample count and reported quantile against raw timings. It also reports observed order and campaign completeness. For compressed logs it reads gzip directly.

Focused HTTP tests covered single/multiple chunks, response limits, timeout and cancellation/permit behavior. Candidate strict Clippy passed in the implementation lane. Release future-size probes in the confirmation measured 592/632/632/592 bytes. No Linux experiment or real multi-chunk latency comparison was performed.
