# BC：每请求新代码的五路请求延迟（WSL2）

本轮为同一机器、同一请求边界的新实验，不能把 [S](wsl-frozen-benchmark.md) 或 [AD](corrected-prefork-benchmark.md) 的 resident / COW 数字拼入本表。任务只测短 scalar 控制和 weighted 七个规模；未测队列、内存或容量，也未更改生产 runtime 或网站。

## 合同与实现

- 控制端 `tools/bc_campaign.py` 先生成各请求的新源码、计算独立 oracle 并序列化。计时从发送请求前开始，到完整响应解码后结束；fresh Python 包含 `Popen`、启动、导入、输入/输出及 `wait`。其他四路用预启动的 JSON-line worker，共同包含传输与 JSON 封装；不能据此单独归因编译或循环性能。传输的只是数百字节源码与行数，Tool 生成输入在各 worker 内的计时期间。源码生成和 oracle 全部在计时外。全部结果均经过 stdout 和 Tool 次数核对，raw 保留实际 stdout、次数、状态、请求序号及耗时。
- MiniPy interpreter/native 每请求从原始源码 prepare/compile，执行现有 `execute_script`、typed QuoteBuffer 和两次 Tool；Monty 每请求 `MontyRun::new`、真实 common 属性 Quote 对象转换和两次 Tool，均不缓存用户代码。Rust 三路各自在 round 开始前启动独立 worker，收到 ready 后才开始请求计时。
- Python 两路共享 `tools/bc_python.py` 的编译/执行/array→namedtuple 转换逻辑。spawn 每次启动 `/usr/bin/python3 -I`，导入公共依赖，从 stdin 接收源码，编译、执行、回传并回收；prefork 父进程只导入公共依赖并给 ready，接收请求后每次 fork，子进程编译/执行/交付，父进程 `waitpid`。ready 之前的初始化另记 `startup.json`，不摊销到单请求。该 prefork 是预热单线程父进程，不是进程池，也没有预编译用户代码；不是安全 sandbox 等价比较。
- 基础脚本为 `fixtures/common_weighted.py` / `common_scalar.py`。每次提交新源码：weighted 的谓词阈值为 `price_limit - 17×variant`，scalar 的表达式结果增加 `variant`；`variant=round×10+sample`，每个 shape 的 30 个请求各有不同的源码（五臂共享同一次请求的源码）。改变的是有效语义而非注释；特定有限行数的 weighted 输出可能碰巧相同，不代表源码相同。每个点三轮 × 十请求，所有五臂按轮和请求交错；round 使用新进程。相同源码与数值参数送至五臂，Python 只作必须的 async function wrapper，MiniPy 原 typed buffer 与对象基线的不同转换都在计时内。这是请求方案的总体比较，不是纯 Python 循环速度或相同隔离语义比较。

## 结果

WSL2 Linux 6.18.54.1、Intel i9-7920X、`taskset -c 2` 串行，独占 `/home/na/benchmarks/minipy-s/compute.lock`；Rust/Cargo 1.96.0，CPython 3.12.3，Monty Git `56949a4c45f51cedc7a96450782f58a8017e114d`，生产源自 main `a1e06cb`；本次仅改控制端 source schedule，复用经 SHA-256 核对的三路 Rust 执行 binary（`ef609fd8…790`）及相同 fixtures/CPython driver。单位 µs，表为 30 个请求的均值；每臂每点 median、min/max、三轮各均值见 `bench/results/bc-wsl/summary.json`，逐请求原始行为同目录 `raw.jsonl`。前次有 `%12` 复用、同 shape 仅 12 种源码的 campaign 完整保存在 `bench/results/bc-wsl-v1/`，不与本轮合并。少量重复不作 p99 推断。

| workload | interpreter | native | Monty | CPython spawn | CPython prefork |
|---|---:|---:|---:|---:|---:|
| scalar | 196 | 571 | 249 | 65,615 | 4,046 |
| weighted 100 | 269 | 879 | 486 | 65,235 | 4,548 |
| weighted 300 | 253 | 862 | 827 | 65,279 | 4,666 |
| weighted 1k | 276 | 884 | 1,995 | 66,558 | 5,354 |
| weighted 3k | 316 | 901 | 5,892 | 68,457 | 6,909 |
| weighted 10k | 498 | 979 | 20,286 | 75,724 | 13,706 |
| weighted 30k | 929 | 1,091 | 64,834 | 91,938 | 30,400 |
| weighted 100k | 2,437 | 1,525 | 274,904 | 157,892 | 98,822 |

这个请求合同下，MiniPy 解释执行在小输入更快，100k 点 native 更快；Python 新进程多数时间是启动成本，prefork 避免每请求启动但仍计入 fork、编译和处理。不能把旧 resident baseline 的毫秒/微秒数直接代入本轮。四个常驻 worker 的启动成本单独记载：三轮 Rust interpreter 约 1.63–2.02 ms、native 1.36–1.48 ms、Monty 1.27–1.44 ms，Python prefork 约 52.6–55.4 ms；不含在请求均值内。fresh spawn 的启动已包含在每次请求中。未限制 Windows 主机负载/SMT 邻核，范围不是置信区间。

## 验证与复现

- `python3 tools/test_bc_campaign.py`：3 tests，包含每个 shape 的 30 种不同源码、每种五臂一致性；WSL 实机五路 smoke 15/15；正式计划 3 rounds × 8 shapes × 10 requests × 5 arms = 1,200/1,200 success。每请求 stdout、0/2 次 Tool、variant 对应关系和 40 组的 round/sample 完整性通过；在 Mac 从复制的 WSL raw 独立调用 `summarize` 回算与 40-cell summary 全等，并核查八个 shape 各有 30 种不同源码。
- 初次 campaign 的构建：WSL 现有 `/home/na/benchmarks/minipy-s/source` 内复用目标目录 `bench/target`，执行 `cargo +1.96.0 build --release --locked --manifest-path bench/Cargo.toml --bin bc_request`（显式 `CARGO_TARGET_DIR=/home/na/benchmarks/minipy-s/source/bench/target`）；binary SHA-256 `ef609fd8d97a7ee3d54a07aa747a46c7ac9bb6a5c85e39e7b4421657cf603790`。首次缺 `bytes 1.11.1`，只下载该 crate 后离线成功。当时同步主仓 `src/`、两个 Cargo lock/manifest、common fixtures 和 BC 文件至既有目录后构建；未创建 clone/worktree。本次修正只同步 `tools/bc_campaign.py`（SHA-256 `5ed2f1af…9ec5`），同一 binary 和 fixture/driver 哈希不变，无需重建。执行 `flock -n /home/na/benchmarks/minipy-s/compute.lock taskset -c 2 python3 tools/bc_campaign.py /home/na/benchmarks/minipy-s/bc-unique-final`（在既有 source 目录，已有 raw 时拒绝覆盖）。新 raw 和 summary 已复制进 Git 路径，WSL 保留原件；旧 `%12` raw 及计时边界修正前仅留 WSL 的两个 `bc-pilot*` 均不混入结果。
