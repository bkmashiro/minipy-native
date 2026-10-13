# Benchmark v2 observations

Raw: v2-raw.jsonl. Commands/status: v2-runs.jsonl. Derived: v2-summary.json.

Commands: {'scheduled': 234, 'completed': 234}

## Latency (median microseconds)

| Arm | Engine | Case | Rows | Lifecycle | Wall us | CPU us |
|---|---|---|---:|---|---:|---:|
| baseline-564381d | monty | stateful_branch | 0 | cold_source_to_result | 55.83 | 56.00 |
| baseline-564381d | monty | stateful_branch | 0 | prepared_request | 16.71 | 17.00 |
| baseline-564381d | monty | stateful_branch | 100 | cold_source_to_result | 119.42 | 120.00 |
| baseline-564381d | monty | stateful_branch | 100 | prepared_request | 83.75 | 85.00 |
| baseline-564381d | monty | stateful_branch | 10000 | cold_source_to_result | 3904.39 | 3905.00 |
| baseline-564381d | monty | stateful_branch | 10000 | prepared_request | 3901.31 | 3902.00 |
| baseline-564381d | monty | weighted_scan | 0 | cold_source_to_result | 56.29 | 57.00 |
| baseline-564381d | monty | weighted_scan | 0 | prepared_request | 21.04 | 21.00 |
| baseline-564381d | monty | weighted_scan | 100 | cold_source_to_result | 116.29 | 116.00 |
| baseline-564381d | monty | weighted_scan | 100 | prepared_request | 78.50 | 79.00 |
| baseline-564381d | monty | weighted_scan | 10000 | cold_source_to_result | 3820.18 | 3820.00 |
| baseline-564381d | monty | weighted_scan | 10000 | prepared_request | 3783.76 | 3783.00 |
| baseline-564381d | native | stateful_branch | 0 | cold_source_to_result | 344.08 | 345.00 |
| baseline-564381d | native | stateful_branch | 0 | prepared_request | 3.92 | 4.00 |
| baseline-564381d | native | stateful_branch | 100 | cold_source_to_result | 347.42 | 348.00 |
| baseline-564381d | native | stateful_branch | 100 | prepared_request | 6.50 | 7.00 |
| baseline-564381d | native | stateful_branch | 10000 | cold_source_to_result | 440.71 | 442.00 |
| baseline-564381d | native | stateful_branch | 10000 | prepared_request | 92.50 | 93.00 |
| baseline-564381d | native | weighted_scan | 0 | cold_source_to_result | 267.00 | 263.00 |
| baseline-564381d | native | weighted_scan | 0 | prepared_request | 3.96 | 4.00 |
| baseline-564381d | native | weighted_scan | 100 | cold_source_to_result | 270.21 | 270.00 |
| baseline-564381d | native | weighted_scan | 100 | prepared_request | 6.46 | 7.00 |
| baseline-564381d | native | weighted_scan | 10000 | cold_source_to_result | 350.17 | 350.00 |
| baseline-564381d | native | weighted_scan | 10000 | prepared_request | 90.58 | 91.00 |
| optimized | monty | stateful_branch | 0 | cold_source_to_result | 55.04 | 55.00 |
| optimized | monty | stateful_branch | 0 | prepared_request | 16.38 | 17.00 |
| optimized | monty | stateful_branch | 100 | cold_source_to_result | 118.38 | 119.00 |
| optimized | monty | stateful_branch | 100 | prepared_request | 80.54 | 82.00 |
| optimized | monty | stateful_branch | 10000 | cold_source_to_result | 3985.06 | 3985.00 |
| optimized | monty | stateful_branch | 10000 | prepared_request | 3869.18 | 3870.00 |
| optimized | monty | weighted_scan | 0 | cold_source_to_result | 53.79 | 54.00 |
| optimized | monty | weighted_scan | 0 | prepared_request | 19.12 | 20.00 |
| optimized | monty | weighted_scan | 100 | cold_source_to_result | 117.63 | 118.00 |
| optimized | monty | weighted_scan | 100 | prepared_request | 81.38 | 82.00 |
| optimized | monty | weighted_scan | 10000 | cold_source_to_result | 3712.72 | 3713.00 |
| optimized | monty | weighted_scan | 10000 | prepared_request | 3898.14 | 3899.00 |
| optimized | native | stateful_branch | 0 | cold_source_to_result | 283.67 | 282.00 |
| optimized | native | stateful_branch | 0 | prepared_request | 3.71 | 4.00 |
| optimized | native | stateful_branch | 100 | cold_source_to_result | 291.00 | 289.00 |
| optimized | native | stateful_branch | 100 | prepared_request | 6.50 | 7.00 |
| optimized | native | stateful_branch | 10000 | cold_source_to_result | 382.92 | 383.00 |
| optimized | native | stateful_branch | 10000 | prepared_request | 93.50 | 94.00 |
| optimized | native | weighted_scan | 0 | cold_source_to_result | 211.33 | 212.00 |
| optimized | native | weighted_scan | 0 | prepared_request | 4.04 | 5.00 |
| optimized | native | weighted_scan | 100 | cold_source_to_result | 209.29 | 209.00 |
| optimized | native | weighted_scan | 100 | prepared_request | 6.25 | 6.00 |
| optimized | native | weighted_scan | 10000 | cold_source_to_result | 289.50 | 290.00 |
| optimized | native | weighted_scan | 10000 | prepared_request | 88.54 | 89.00 |

## Finite load observations

| Arm | Engine | Lifecycle | Concurrency | Delay us/op | Requests/s | Short p99 us |
|---|---|---|---:|---:|---:|---:|
| baseline-564381d | monty | prepared_shared_code_bounded_admission | 1 | 0 | 942.6 | 4400.93 |
| baseline-564381d | monty | prepared_shared_code_bounded_admission | 1 | 1000 | 172.6 | 23150.22 |
| baseline-564381d | monty | cold_fixed_arrival_bounded_admission | 8 | 0 | 965.7 | 8182.12 |
| baseline-564381d | monty | prepared_shared_code_bounded_admission | 8 | 0 | 901.9 | 38496.52 |
| baseline-564381d | monty | cold_fixed_arrival_bounded_admission | 8 | 1000 | 564.8 | 59248.72 |
| baseline-564381d | monty | prepared_shared_code_bounded_admission | 8 | 1000 | 617.8 | 53736.83 |
| baseline-564381d | monty | prepared_shared_code_bounded_admission | 32 | 0 | 1025.8 | 124951.19 |
| baseline-564381d | monty | prepared_shared_code_bounded_admission | 32 | 1000 | 927.7 | 141498.80 |
| baseline-564381d | native | prepared_shared_code_bounded_admission | 1 | 0 | 30621.8 | 128.67 |
| baseline-564381d | native | prepared_shared_code_bounded_admission | 1 | 1000 | 194.7 | 21996.75 |
| baseline-564381d | native | cold_fixed_arrival_bounded_admission | 8 | 0 | 1000.6 | 2268.09 |
| baseline-564381d | native | prepared_shared_code_bounded_admission | 8 | 0 | 41093.6 | 683.84 |
| baseline-564381d | native | cold_fixed_arrival_bounded_admission | 8 | 1000 | 941.5 | 7349.18 |
| baseline-564381d | native | prepared_shared_code_bounded_admission | 8 | 1000 | 1799.8 | 18030.07 |
| baseline-564381d | native | prepared_shared_code_bounded_admission | 32 | 0 | 43718.8 | 2560.93 |
| baseline-564381d | native | prepared_shared_code_bounded_admission | 32 | 1000 | 6443.8 | 20257.12 |
| optimized | monty | prepared_shared_code_bounded_admission | 1 | 0 | 939.9 | 4315.64 |
| optimized | monty | prepared_shared_code_bounded_admission | 1 | 1000 | 176.4 | 22682.76 |
| optimized | monty | cold_fixed_arrival_bounded_admission | 8 | 0 | 955.5 | 18408.64 |
| optimized | monty | prepared_shared_code_bounded_admission | 8 | 0 | 864.3 | 37472.52 |
| optimized | monty | cold_fixed_arrival_bounded_admission | 8 | 1000 | 606.5 | 57451.33 |
| optimized | monty | prepared_shared_code_bounded_admission | 8 | 1000 | 645.5 | 52275.20 |
| optimized | monty | prepared_shared_code_bounded_admission | 32 | 0 | 1025.7 | 126538.66 |
| optimized | monty | prepared_shared_code_bounded_admission | 32 | 1000 | 926.7 | 140003.59 |
| optimized | native | prepared_shared_code_bounded_admission | 1 | 0 | 29465.7 | 126.58 |
| optimized | native | prepared_shared_code_bounded_admission | 1 | 1000 | 204.1 | 21255.92 |
| optimized | native | cold_fixed_arrival_bounded_admission | 8 | 0 | 988.0 | 2520.17 |
| optimized | native | prepared_shared_code_bounded_admission | 8 | 0 | 34403.8 | 877.71 |
| optimized | native | cold_fixed_arrival_bounded_admission | 8 | 1000 | 928.5 | 7982.95 |
| optimized | native | prepared_shared_code_bounded_admission | 8 | 1000 | 1653.7 | 19525.49 |
| optimized | native | prepared_shared_code_bounded_admission | 32 | 0 | 44113.7 | 2483.84 |
| optimized | native | prepared_shared_code_bounded_admission | 32 | 1000 | 6366.2 | 19885.16 |

## Memory

See summary memory phase observations, task_slopes and byte_costs. Fits are diagnostic observations, not confidence intervals. Logical 10B is not RSS 10B.

## Limits

- Speed uses default uninstrumented System allocator; memory is diagnostic and not a latency arm.
- CPU getrusage is process-wide with microsecond resolution.
- RSS/PSS are sampled current memory, peak RSS is whole-process getrusage. Requested allocator bytes exclude mmap/JIT/metadata.
- Small physical payload differences may be below page/allocator resolution; negative sampled slopes are not clamped.
- Native bytes is a touched Vec<u8> ownership probe; native str/bytes language unsupported.
- Request percentiles within one concurrent run are not independent run error bars.
