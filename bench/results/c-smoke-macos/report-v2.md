# Benchmark v2 observations

Raw: v2-raw.jsonl. Commands/status: v2-runs.jsonl. Derived: v2-summary.json.

Commands: {'scheduled': 17, 'completed': 17}

## Latency (median microseconds)

| Arm | Engine | Case | Rows | Lifecycle | Wall us | CPU us |
|---|---|---|---:|---|---:|---:|
| current | monty | stateful_branch | 100 | cold_source_to_result | 183.21 | 183.50 |
| current | monty | stateful_branch | 100 | prepared_request | 87.31 | 88.50 |
| current | monty | weighted_scan | 100 | cold_source_to_result | 239.52 | 240.00 |
| current | monty | weighted_scan | 100 | prepared_request | 115.98 | 116.50 |
| current | native | stateful_branch | 100 | cold_source_to_result | 429.54 | 430.50 |
| current | native | stateful_branch | 100 | prepared_request | 3.50 | 4.00 |
| current | native | weighted_scan | 100 | cold_source_to_result | 443.87 | 444.00 |
| current | native | weighted_scan | 100 | prepared_request | 3.88 | 5.50 |

## Finite load observations

| Arm | Engine | Lifecycle | Concurrency | Delay us/op | Requests/s | Short p99 us |
|---|---|---|---:|---:|---:|---:|
| current | monty | prepared_shared_code_bounded_admission | 2 | 0 | 772.9 | 10323.08 |
| current | native | prepared_shared_code_bounded_admission | 2 | 0 | 45112.9 | 162.46 |

## Memory

See summary memory phase observations, task_slopes and byte_costs. Fits are diagnostic observations, not confidence intervals. Logical 10B is not RSS 10B.

## Limits

- Speed uses default uninstrumented System allocator; memory is diagnostic and not a latency arm.
- CPU getrusage is process-wide with microsecond resolution.
- RSS/PSS are sampled current memory. Current Linux observed HWM is cumulative max(sampled smaps RSS, reported VmHWM), with raw kernel readings retained; it is not an exact continuously traced peak. macOS and historical probes use getrusage; see each raw physical.method. Requested allocator bytes exclude mmap/JIT/metadata.
- Small physical payload differences may be below page/allocator resolution; negative sampled slopes are not clamped.
- Native bytes is a touched Vec<u8> ownership probe; native str/bytes language unsupported.
- Request percentiles within one concurrent run are not independent run error bars.
