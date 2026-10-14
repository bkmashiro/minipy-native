# AF: open-loop source-once capacity

## Result and scope

Five engines, two representative workloads, three offered-rate points and three independent process rounds: 90/90 confirmations. All 870,063 offered requests reconcile as 825,408 completed and 44,655 rejected, with no failed or timed-out requests. Another 462,522 planned requests were cancelled before offering after bounded overload stops. Every completed stdout matches an independent integer oracle; Tool counts, monotonic timestamp order and all raw-to-summary fields match exactly.

Same WSL x86-64 host as AD. Service process tree pinned to CPU2, driver to separate physical CPU4, unused SMT siblings 3/5, GOMAXPROCS=1, exclusive shared compute.lock. Source/runtime/binary identities are in `bench/results/af-capacity/identity*.json`; 41 postflight hashes matched. Production runtime and Guest were not changed. The benchmark adapters add only streaming dispatch through the existing execution paths.

| Engine | CPU sustained / overload req/s | Mixed sustained / overload req/s |
|---|---:|---:|
| MiniPy typed IR | 3,800 / 5,000 | 5,000 / 7,500 |
| MiniPy native | 1,450 / 1,850 | 1,900 / 2,600 |
| Monty objects | 45 / 65 | 40 / 70 |
| CPython per-request prefork JSON | 30 / 40 | 45 / 65 |
| Pysolate COW JSON | 6 / 9 | 10 / 12 |

These are the largest tested rates passing the finite-window screen in all three rounds and the first higher tested rate showing overload. They bracket observed behavior, not an exact or indefinite maximum. In particular, COW mixed 12 has no rejection but accumulating completion debt, so absence of rejection alone is insufficient. Values compare complete system adapters, not equal memory representation or equal isolation strength. No cross-cohort strict speed ratio is asserted.

At the sustainable load, three-round mean p99 response latency is CPU IR/native/Monty/prefork/COW 2.706/3.790/21.978/43.215/149.932 ms; mixed 9.950/10.766/544.388/594.378/3313.170 ms. These are percentiles of all completed requests in each workload, not AD's short-task-only percentiles. Mixed COW includes long stateful100k tasks and therefore has a long tail even at a stable offered rate. See JSON for mean/min/max, each round and the higher-load curves.

## Measurement contract

CPU: weighted 10,000 rows, no Tool delay. Mixed: forty-request period with 24 scalar, 8 weighted100, 4 stateful100, 2 weighted10000 and 2 stateful100000, permuted by `(id*17)%40`; each of the two Tools waits 1ms. All paths prepare/compile source per request, execute actual source logic, return stdout and recycle task state. Runtime initialization happens once before a run and is not repeated per request. No prepared code cache. Fixed source fixture loading and independent oracle construction are outside the measured request, consistent with corrected AD; user source compilation and actual input generation/adaptation are inside service work.

External FIFO admits up to 64 waiting and 16 dispatched in-flight requests. This is common benchmark admission, not production runtime admission or a fairness guarantee. Constant-rate planned CLOCK_MONOTONIC arrivals are independent of completions. End-to-end latency starts at the PLANNED arrival and ends when the driver observes the response, including driver lateness, FIFO, IPC and runtime dispatch. Queue ends when worker execution starts; service ends after actual execution. Response serialization/observation is in end-to-end latency, not service latency. The driver can reject overflow instead of retaining unlimited requests.

Each confirmation offers arrivals for 15s with first 3s warm-up excluded from latency and completion-rate statistics. No periodic drain/reset. If waiting queue >=48 at three consecutive 1Hz samples and elapsed >=warmup+3s, future arrivals stop; outstanding requests drain with a 30s limit. Throughput counts actual response completions during the measurement window, excluding drain. Latency uses requests planned in that window, including their drain completions. Missing/censored latency stays missing. Plot conditional completed p99 together with rejection/failure/timeout counts and early stops: discarded requests must never improve a chart invisibly.

CPU includes service Host, native parent and monotonic cumulative reaped-child CPU, plus drain, divided by completed requests across the run. Initialization excluded. Driver CPU is separate per offered request. Low-rate driver CPU/request includes idle polling and is not service cost. At high sustainable IR CPU/mixed load it is 0.040/0.058 ms/offered (roughly 15/29% of its separate core). Maximum independent-run p99 arrival lateness across all points is 1.315ms, without growing historical trace work. This validates the tested generator rates, not an unlimited generator capacity.

## Stability and validity corrections

`sustained_v2` is one rule applied to every engine: full >=8s measurement window, no early stop/reject/error/timeout, all measurement-arrival requests completed, actual completion rate >=98% of offered, backlog slope <=max(0.5,1% offered) requests/s, last-third p99 <=1.5*first-third p99+2ms, mean final three WAITING queue samples <=max(2,5% offered). Thresholds are engineering screening choices, not an SLA or statistical proof. Percentiles are nearest rank per independent run; aggregate their mean/min/max, never pool requests as independent rounds.

Original `stable_observed` (v1) is retained in every run. Its final threshold incorrectly counted natural in-flight awaits as waiting debt. v2 changes only that final queue variable, retaining total backlog slope, completion debt and latency trend checks for all engines. Prefork mixed45 and COW mixed10 can pass v2 without erasing overload at higher loads.

The original driver rescanned all accumulated requests twice at each 1Hz trace, causing O(N) high-rate stalls. Only four affected round0 high-rate MiniPy CPU points were replaced. Originals remain in canonical `excluded-linear-trace`; 11 low-rate round0 points (<=675 requests) were valid and reused, all other confirmations use constant-time counters. A dispatch timestamp race in the first pilot was corrected by sampling before atomic pipe write, before any affected pilot result was persisted. No production change or full-matrix rerun.

## Storage, cost and reproduction

Canonical request raw remains ONE gzip copy on WSL at `/home/na/benchmarks/minipy-s/af-capacity`, indexed by sha256, counts and compressed size in `raw-index.json`. Confirmation raw is 41.84 MB. Git contains ~0.63 MB of small summaries/index/identities/plan, not raw duplicates or build artifacts. `import-summary.json` contains 30 plot-ready points, 90 independent-run summaries, contract, counts and capacity observations. Existing AD/S/W resident, Rust/ASM full and prepared evidence remains unchanged; it is not spliced into these source-once capacity curves.

Measured confirmations spend 1,164.01s offering arrivals and 68.87s draining (20.55min total), versus 1,350s nominal arrival budget. Runtime initialization/launch/reap is additional and not individually timed; COW initialization is outside request timers. Initial actual build was 6.3s; full raw reduction was 11.70s. Pilot, smoke, four replaced points, diagnostic analysis and the interrupted controller increase total wall time; do not infer their duration from confirmation totals. Collection, not build or reduction, dominates the measured budget. Constant-time trace and gzip/index storage are the only infrastructure simplifications needed; no new framework or Guest rebuild.

From this repo, copy `tools/af_capacity.py` and `tools/af_reduce.py` alongside the frozen measured workers. Existing workers were built from `bench/src/bin/interpreter_ab.rs` + `bench/src/af_ipc.rs` and the AD Go runner + `tools/af_service.go`.

```sh
cd /home/na/benchmarks/minipy-s
# Continue ONLY missing summaries; run each workload/round batch separately.
AF_WORKLOAD=mixed flock -n compute.lock taskset -c 4 python3 af-capacity/af_capacity.py /home/na/benchmarks/minipy-s confirm 2
python3 af-capacity/af_identity.py /home/na/benchmarks/minipy-s
python3 af-capacity/af_reduce.py /home/na/benchmarks/minipy-s/af-capacity /home/na/benchmarks/minipy-s/af-capacity/export
# Local arithmetic checks, not synthetic performance evidence:
python3 -m unittest discover -s tools -p test_af_capacity.py
```

Web handoff: import this new cohort separately; show engine representation labels and all loss/early-stop information. Preserve prior latency/memory data and the landing cards. The load curve has three measured x positions per engine, not an interpolated precision capacity estimate. Website changes and deployment belong to AG, not this experiment.
