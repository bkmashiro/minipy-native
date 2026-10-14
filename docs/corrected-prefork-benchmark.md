# AD：修正请求计时与 JSON 对象 prefork 对照

## 本轮问题与范围

问题：在源码预载、oracle 全部移出计时后，prepared DataImage COW 的实际请求成本与 import-prewarmed CPython 每请求 fork/执行/退出/回收有何差异？

本轮只改 benchmark，不改 MiniPy/Pysolate 生产 runtime，不移植历史 poll1024/static-table 优化，不改变取消、语言或预算语义。原 S/W 数据和适配器保留。网站由后续 AE 卡整合，本卡不修改网站。

本卡相对分支基点`8d19e69`的`src/`及Cargo文件diff为空。当前仓库在该基点已经与原S的23文件manifest有6处差异（Cargo.toml/lock、interpreter.rs、interpreter/tape.rs、lib.rs、runtime.rs）；本轮只读核对不能作为旧S源码在HEAD仍冻结的通过证明。其余引擎保留原`runtime_commit=969e679`和cohort，不按本卡最终evidence commit重新标记运行版本，也未重新测试它们。

## 正式 v2 结果

300/300命令成功，11,769测量行及300初始化行。13个source/artifact文件和300原逐command raw在WSL验收；本机从12,069聚合raw逐值复算全部四类summary一致。三独立process rounds，以下为等请求数均值，单位ms；完整三轮范围在JSON。

| workload | COW wall | prefork wall | COW/prefork | COW CPU | prefork CPU |
|---|---:|---:|---:|---:|---:|
| scalar 0 | 3.092 | 3.221 | 0.960× | 3.093 | 3.221 |
| weighted 10,000 | 139.313 | 31.437 | 4.432× | 139.887 | 31.486 |
| stateful 10,000 | 132.182 | 30.996 | 4.264× | 132.005 | 30.915 |
| weighted 100,000 | 1403.057 | 223.674 | 6.273× | 1405.119 | 222.861 |
| stateful 100,000 | 1322.705 | 217.748 | 6.074× | 1325.342 | 217.044 |

scalar量级接近，本cohort COW wall约低4%；weighted/stateful 10k约慢4.3–4.4倍，100k约慢6.1–6.3倍。因此校正计时并对齐JSON/对象表示后，大扫描差距仍在；本轮没有分项CPU占比，不能把它直接归因于某个未移植优化或单个copy。JSON resident scalar/weighted10k为0.539/22.907ms，另列，不能作为每请求隔离的同义baseline。

Mixed（两次Tool各1ms）的light窗口：COW/prefork CPU为78.988/17.244ms/task，有限吞吐12.67/55.09req/s；短任务p99为274.222/725.564ms。scan窗口分别为157.016/31.532ms/task、6.36/30.14req/s，短p99为484.974/955.101ms。COW本组短任务尾延迟较低，但CPU及吞吐较差，不能用一个维度判整体胜负。400req/s是到达计划上限。

32个weighted10k的record_wait whole-tree PSS：COW672.795MiB，prefork391.733MiB；各自匹配基线为396.024/106.161MiB，增量276.771/285.572MiB。fork基线含同一批空载child，COW基线在实例创建前，绝对值与增量须同时保留该生命周期标签。reclaimed表示请求全结束/child已reap，并不保证Go allocator已还页，不能单凭高于startup判断泄漏。

现有1/8/32 counts已经可供AE按独立round/相同phase/code/input拟合允许截距的KiB/task斜率，无需新benchmark。按三轮均值record_wait点拟合的描述性斜率：COW8629.353、prefork9103.163、resident6187.623、fresh29519.248KiB/task；不是输入行数斜率，亦非纯Python对象字节。JSON传输字节不应拿来冒充逻辑raw-input owner或严格物理下界。

同运行first scalar launch-through-reap wall：COW7.460s，其中runtime初始化7.414s、执行3.612ms；prefork76.764ms，其中初始化61.758ms、执行4.292ms。各行五段精确相加；稳态3ms不是首次启动。

已按最新主控决定收敛当前v2，剩余benchmark为0，不再增量补测、另开campaign或新增审计。本卡不修改/部署网站，精确替换清单交AE。

## 比较契约

- 实机：同一 WSL2 x86_64，Intel i9-7920X；每命令串行，Go host、native parent 和所有 fork child 继承 `taskset -c 2`。构建、smoke、正式 campaign 持同一 `/home/na/benchmarks/minipy-s/compute.lock`，同时兼容旧 `wsl-heavy.lock`。Windows 活动、物理 SMT 竞争未受控。
- Pysolate 使用已存在 W 的 8 MiB Tool-response Guest 与 task-local runtime copy，CPython 3.14.0 / 上游 wazero v1.11.0 compiler。没有重新构建 Guest 或修改 W runtime。native 使用 WSL CPython 3.12.3。因此这是实际路径对照，并未隔离 Python 版本或纯引擎成本。
- 四臂：`pysolate-cow`、`pysolate-fresh`、`cpython-fork-json`、`cpython-resident-json`。COW 每请求创建/关闭实例；Wasm compile、CPython 初始化与 DataImage capture 为 resident setup。native fork 的单线程父进程预载 stdlib/source 并 `gc.freeze`，每请求真正 `os.fork`，child 重新 compile/exec，输出经过 pipe，`os._exit` 后父 `waitpid`，请求返回之前已经 reap。不是 worker pool。
- 原 S `tools/final_python.py:120–155` 同样是每请求 fork+新 event loop+compile+exit/waitpid，但 packed-array/namedtuple、Python producer 与本轮 JSON 组不同，保留为原 common 数据，不冒充新的 matched baseline。

### 输入、适配与实际传输

两侧调用同一 Go Manifest 回调。每个 Tool 请求内生成 N 个 map：`unit_price_cents=(i*37)%1000`，`quantity=i%9-2`；limit=700，batch=17（mixed 以 batch=17+N 传递请求行数给 shared Host）。Host 将 map 列表编码为 compact `{"value":...}` JSON；Python json.loads 先得到 dict，再在 user source 中逐行 `SimpleNamespace(**q)`。没有预生成 payload、复用对象或省掉 native 的隔离传输。

Pysolate 使用生产 Tool ABI 的 Guest buffer / Host 写入 / PyBytes / Guest JSON；native 使用真实 localhost TCP JSON Tool RPC，fork child 另有 parent control 请求与 pipe 输出。TCP newline framing 与 WASI/ABI 不是同一种传输，全部留在请求成本内。native resident 使用 async TCP Tool 调用以允许真实等待并发，源码预载时套 async function wrapper；无每请求子进程隔离。

Host 源码文件读取、同步 Tool 适配和 resident async wrapper 构造位于请求外。用户 Python `compile` 在每请求执行内，三者均不复用 user code object。`runner-shared` 的 native 行表示 parent/Host/source 共享，并不表示已编译代码共享。

### 计时与 CPU

- 稳态 speed：从调用生产 Runner 或 native control 请求之前，到输出返回、实例关闭或 fork child 已 reap；含 user compile、输入生成、JSON、跨隔离传输、对象转换、扫描和输出。runtime/process 初始化单列。
- Mixed：每窗口 40 个请求，计划到达间隔 0.5 ms，名义窗口 100 ms，inflight=40；light 权重24/8/4/2/2，scan权重8/16/8/4/4；两次 Tool 各延迟0或1ms。请求 latency 截止真实输出/reap；整个窗口 wall/CPU 截止全体完成。之后才做 O(N) oracle 和 Tool 次数检查，不计入请求或窗口指标。
- First：controller 从 fresh worker launch 到 worker/其子树回收；同运行记录启动、runtime init、Host source preparation、execute、response/reap/unassigned 五段。源码准备不等于 user compile。first 的输出和 Tool oracle 在 controller 获取 wall/CPU 终点后校验。
- Go `RUSAGE_SELF` 加从内核读取的 native parent process `CPUCLOCK_SCHED`，再加每请求 waitpid 后通过正常run响应返回的累计 `RUSAGE_CHILDREN`；Go使用原子max处理并发响应乱序。CPU采样不发送RPC，不唤醒native parent，也不使用10ms粒度的/proc jiffies。first全进程CPU仍由controller对已回收worker树计量。
- 吞吐为有限 burst 的 `requests / sum(max(100ms, drain_wall))`，400 req/s 是到达窗口上限；不是引擎最大吞吐。

### 重复与归约

15 个 speed 形状：scalar 与 weighted/stateful 各100、300、1k、3k、10k、30k、100k。每个 cell 三个独立 worker process，臂顺序按 round 旋转；每 process 11 请求，前1个丢弃，保留10个。Mixed 每 process 5 windows，前1个丢弃。memory 为1/8/32个weighted10k；first 为scalar/weighted10k/stateful10k。仅这四臂重测，其他引擎不重跑。

计划300 commands、11,769正式 trace 行，另300 initialization；其中9行是 fork `empty_tree`。请求数不当独立run，展示三轮均值范围，不合成CI。CPU每窗口只计一次，p50/p95/p99使用nearest-rank，warmup原始行保留。

### 进程树 PSS 与匹配空载

native whole tree 包括 Go Host、prewarmed parent、所有存活 fork child，RSS/PSS均逐PID加总。raw 含每个PID和各自值；fork请求完成后验证children=0。

fork 的 `startup` 只有 Host+parent，不能作为 living child 的空载基线。新增 `empty_tree`：同一批已 fork、尚未compile/调用Quote Tool的children在内部gate等待，空载、load_wait、record_wait的PID集合严格相同。load/record增量按每round减该 `empty_tree`，reclaimed按已reap后的原controller `startup`。其他三臂使用同一PID树startup。归约提供每行 `baseline_phase`、`baseline_pss_bytes`、`incremental_pss_bytes`；不能给fork等待行套旧的通用stage-minus-controller-startup公式。

这些增量还保留生命周期差异：fork的空载已经包含child创建，COW/fresh的startup在请求实例创建前。whole-tree绝对PSS包含各自固定与实例成本；增量回答各自匹配基线之上的增长，不能称为已完全归一化的纯Quote对象字节或纯引擎内存。两者应同时保留并标明基线。

## 验证与审查

- WSL实际构建（现有Go1.27.1，无网络下载）、5个Go tests，包括真实native新child PID、不复用pool、返回前已reap、producer/JSON/Tool次数及CPU累计；新增并发响应计数单调性、control socket不可用时仍可采CPU、确实计入reaped child CPU，并与native诊断RUSAGE对照。
- 8个确定性Python plan/accounting tests，覆盖300/11769计划、first加性/缺行/输出/Tool oracle拒绝、旧cohort拒绝、fork空载PID配对和PSS逐PID加总，以及mixed CPU不重复计入每请求、ns/µs换算、有限窗口吞吐和缺失阶段保留null。
- 四臂各 speed100k、mixed scan、hold8、first10k，共16项真实WSL smoke通过。first五段纳秒逐行精确相加。
- `gpt-6-luna` 独立只读审阅固定签名提交 `660cd5e`，发现first整进程计时仍包含worker oracle。已在`37408e9`移到controller reap后校验，重新通过tests和16项smoke，再启动正式campaign。没有额外全审计或修改生产runtime。
- 第一轮完整300命令的验收发现native scalar CPU探针污染：wall约2.945ms、CPU约3.535ms。native采样RPC在wall外而在CPU差值内，且请求前探针会唤醒parent。整轮保留在`bench/results/ad-probe-overhead-pilot/`，明确排除，不拼入正式结果。在`1c8d9ff`改为上述内核parent CPU clock + normal run响应的reaped child累计值，重新5项Go tests及16项smoke，通过后重跑完整四臂三轮，正式cohort为`ad-corrected-json-prefork-v2`。实测校准kernel parent+child为111.6565ms、诊断RUSAGE为112.1000ms，差0.4435ms包含额外诊断RPC本身；测试2ms拒绝阈值是工程校验容差，不是clock精度或结果误差条。

## 复现与交付

依赖已有 W task-local runtime/Guest（identity与旧结果见Pysolate Spine仓库 `experiments/minipy-w-guest`）及 `docs/wsl-frozen-benchmark.md` 描述的 S fixture/toolchain，不下载新runtime。源码位于 `tools/ad_{pysolate.go,native.go,native.py,campaign.py,reduce.py}`。

1. 在 WSL `ad-corrected/harness` 复用W的go.mod/go.sum（replace指向W task-local runtime），copy `ad_pysolate.go`→`main.go`，`ad_native.go`→`native.go`，`ad_native_test.go`→`native_test.go`；copy `ad_native.py`→`../native.py`，`ad_campaign.py`→`../campaign.py`。
2. 持共享compute.lock与wsl-heavy.lock，`GOPROXY=off go build -o ../worker .`；`AD_NATIVE_SCRIPT=../native.py GOPROXY=off go test -v .`。
3. 同一锁内，`python3 ../campaign.py /home/na/benchmarks/minipy-s smoke`，再设置当前源码`AD_SOURCE_COMMIT`运行`formal`。每command结果独立持久化，resume只接受完整匹配的source/artifact identity，不拼入旧cohort。
4. `ad_reduce.py`放在同目录，配套`ad_campaign.py`导入名指向当前campaign；`python3 ad_reduce.py BASE`。它验证300commands全部完成、每cell三轮、raw计数、warmup、first分段、mixed形状/CPU、memory PID树和所有输入文件hash，再输出AE可导入`import-summary.json`与`replacement.json`。

原逐command raw、targets与runtime留在WSL；仓库交付小型聚合raw gzip、计划、identity、归约、校验与报告。导出后可在macOS或Linux运行 `python3 tools/ad_reduce.py --replay bench/results/ad-wsl`，重新校验全部300commands/12,069原始行并逐值复算四类summary；此offline replay不冒充远端artifact重新验收。AE必须整臂删除旧Pysolate curve/mixed/memory/first，插入AD完整四臂；新JSON prefork设主要baseline，JSON resident单列可选，旧namedtuple CPython继续保留原身份。四臂属于`ad-json-object`配对组，不能暗中并成所有引擎同输入表示的排名。
