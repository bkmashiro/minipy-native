# Benchmark v2 observations

Raw: v2-raw.jsonl. Commands/status: v2-runs.jsonl. Derived: v2-summary.json.

Commands: {'scheduled': 17, 'completed': 17}

## Latency (median microseconds)

| Arm | Engine | Case | Rows | Lifecycle | Wall us | CPU us |
|---|---|---|---:|---|---:|---:|
| current | monty | stateful_branch | 100 | cold_source_to_result | 181.12 | 181.50 |
| current | monty | stateful_branch | 100 | prepared_request | 85.29 | 85.50 |
| current | monty | weighted_scan | 100 | cold_source_to_result | 220.40 | 221.00 |
| current | monty | weighted_scan | 100 | prepared_request | 102.94 | 103.00 |
| current | native | stateful_branch | 100 | cold_source_to_result | 472.77 | 473.50 |
| current | native | stateful_branch | 100 | prepared_request | 3.38 | 3.50 |
| current | native | weighted_scan | 100 | cold_source_to_result | 444.38 | 445.00 |
| current | native | weighted_scan | 100 | prepared_request | 3.65 | 3.50 |

## Concurrent prepared requests

| Arm | Engine | Concurrency | Delay us/op | Requests/s | Short p99 us |
|---|---|---:|---:|---:|---:|
| current | monty | 2 | 0 | 779.4 | 10238.21 |
| current | native | 2 | 0 | 44056.7 | 166.04 |

## Memory

See summary memory phase observations, task_slopes and byte_costs. Fits are diagnostic observations, not confidence intervals. Logical 10B is not RSS 10B.

## Limits

- Speed uses default uninstrumented System allocator; memory is diagnostic and not a latency arm.
- CPU getrusage is process-wide with microsecond resolution.
- RSS/PSS are sampled current memory, peak RSS is whole-process getrusage. Requested allocator bytes exclude mmap/JIT/metadata.
- Small physical payload differences may be below page/allocator resolution; negative sampled slopes are not clamped.
- Native bytes is a touched Vec<u8> ownership probe; native str/bytes language unsupported.
- Request percentiles within one concurrent run are not independent run error bars.
