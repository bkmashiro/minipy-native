# BC：每请求新代码的五路请求延迟（WSL2）

本轮为同一机器、同一请求边界的新实验，不能把 [S](wsl-frozen-benchmark.md) 或 [AD](corrected-prefork-benchmark.md) 的 resident / COW 数字拼入本表。任务只测短 scalar 控制和 weighted 七个规模；未测队列、内存或容量，也未更改生产 runtime 或网站。

## 合同与实现

- 控制端 `tools/bc_campaign.py` 先生成各请求的新源码、计算独立 oracle 并序列化。计时从发送请求前开始，到完整响应解码后结束；fresh Python 包含 `Popen`、启动、导入、输入/输出及 `wait`。其他四路用预启动的 JSON-line worker，共同包含传输与 JSON 封装；不能据此单独归因编译或循环性能。传输的只是数百字节源码与行数，Tool 生成输入在各 worker 内的计时期间。源码生成和 oracle 全部在计时外。全部结果均经过 stdout 和 Tool 次数核对，raw 保留实际 stdout、次数、状态、请求序号及耗时。
- MiniPy interpreter/native 每请求从原始源码 prepare/compile，执行现有 `execute_script`、typed QuoteBuffer 和两次 Tool；Monty 每请求 `MontyRun::new`、真实 common 属性 Quote 对象转换和两次 Tool，均不缓存用户代码。Rust 三路各自在 round 开始前启动独立 worker，收到 ready 后才开始请求计时。
- Python 两路共享 `tools/bc_python.py` 的编译/执行/array→namedtuple 转换逻辑。spawn 每次启动 `/usr/bin/python3 -I`，导入公共依赖，从 stdin 接收源码，编译、执行、回传并回收；prefork 父进程只导入公共依赖并给 ready，接收请求后每次 fork，子进程编译/执行/交付，父进程 `waitpid`。ready 之前的初始化另记 `startup.json`，不摊销到单请求。该 prefork 是预热单线程父进程，不是进程池，也没有预编译用户代码；不是安全 sandbox 等价比较。
- 基础脚本为 `fixtures/common_weighted.py` / `common_scalar.py`。每次提交新源码：weighted 的谓词阈值为 `price_limit - 17×variant`，scalar 的表达式结果增加 `variant`；`variant=(round×10+sample)%12`。改变的是有效语义而非注释，每个点三轮 × 十请求，所有五臂按轮和请求交错；round 使用新进程。相同源码与数值参数送至五臂，Python 只作必须的 async function wrapper，MiniPy 原 typed buffer 与对象基线的不同转换都在计时内。这是请求方案的总体比较，不是纯 Python 循环速度或相同隔离语义比较。

## 结果

WSL2 Linux 6.18.54.1、Intel i9-7920X、`taskset -c 2` 串行，独占 `/home/na/benchmarks/minipy-s/compute.lock`；Rust/Cargo 1.96.0，CPython 3.12.3，Monty Git `56949a4c45f51cedc7a96450782f58a8017e114d`，生产源自 main `a1e06cb`。单位 µs，表为 30 个请求的均值；每臂每点 median、min/max、三轮各均值见 `bench/results/bc-wsl/summary.json`，逐请求原始行为同目录 `raw.jsonl`。少量重复不作 p99 推断。

| workload | interpreter | native | Monty | CPython spawn | CPython prefork |
|---|---:|---:|---:|---:|---:|
| scalar | 229 | 612 | 260 | 65,314 | 4,134 |
| weighted 100 | 267 | 895 | 499 | 65,923 | 4,644 |
| weighted 300 | 265 | 885 | 875 | 65,929 | 4,719 |
| weighted 1k | 283 | 896 | 2,036 | 66,939 | 5,279 |
| weighted 3k | 326 | 935 | 5,945 | 69,121 | 7,150 |
| weighted 10k | 490 | 987 | 19,723 | 75,602 | 13,876 |
| weighted 30k | 978 | 1,122 | 66,112 | 93,450 | 31,118 |
| weighted 100k | 2,526 | 1,555 | 282,010 | 159,088 | 100,231 |

这个请求合同下，MiniPy 解释执行在小输入更快，100k 点 native 更快；Python 新进程多数时间是启动成本，prefork 避免每请求启动但仍计入 fork、编译和处理。不能把旧 resident baseline 的毫秒/微秒数直接代入本轮。四个常驻 worker 的启动成本单独记载：三轮 Rust interpreter 约 1.66–2.14 ms、native 1.37–1.39 ms、Monty 1.33–1.40 ms，Python prefork 约 52.7–53.2 ms；不含在请求均值内。fresh spawn 的启动已包含在每次请求中。未限制 Windows 主机负载/SMT 邻核，范围不是置信区间。

## 验证与复现

- `python3 tools/test_bc_campaign.py`：3 tests；WSL 实机五路 smoke 15/15；正式计划 3 rounds × 8 shapes × 10 requests × 5 arms = 1,200/1,200 success。每请求 stdout、0/2 次 Tool 和 40 组的 round/sample 完整性通过；在 Mac 从 Git raw 独立调用 `summarize` 回算与提交的 40-cell summary 全等。
- WSL 现有 `/home/na/benchmarks/minipy-s/source` 内复用目标目录 `bench/target`，仅需 `cargo +1.96.0 build --release --locked --manifest-path bench/Cargo.toml --bin bc_request`（显式 `CARGO_TARGET_DIR=/home/na/benchmarks/minipy-s/source/bench/target`）；binary SHA-256 `ef609fd8d97a7ee3d54a07aa747a46c7ac9bb6a5c85e39e7b4421657cf603790`。首次缺 `bytes 1.11.1`，只下载该 crate 后离线成功。现有 WSL source 曾是冻结旧版；本次将主仓 `src/`、两个 Cargo lock/manifest、common fixtures 和 BC 文件同步至既有目录后构建；未创建 clone/worktree。运行：`flock -n /home/na/benchmarks/minipy-s/compute.lock taskset -c 2 python3 tools/bc_campaign.py /home/na/benchmarks/minipy-s/bc-final`（已有 raw 时拒绝覆盖，换新输出目录）。原始 raw 已在 Git 小文件，WSL 保留一份；计时边界修正前的两次 pilot 仅留 WSL `bc-pilot*`，不混入结果。
