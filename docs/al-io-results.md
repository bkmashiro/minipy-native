# AL：I/O-enabled 调度成本归因与负结果

## 结论

不保留性能修改。唯一候选是 Host 的 Tokio current-thread `event_interval(256)`，基线默认 61；两侧均启用真实 I/O，保留 MiniPy 默认 chunk=256、所有 fairness yield、取消、预算和 owner 语义。小配对没有稳定端到端净收益，停止，没有第二项候选。生产 `src/`、Cargo manifest/lock 与批准 AJ `a780931698e29553259411c08a96a2612b9c8b53` 完全相同；它包含 AH `f7167add303a52de73f449d1ef5c57df9552e2f4`。

这轮更重要的结论是细化 AH/AJ 的归因：I/O-enabled ready scan 的约14 µs 额外成本确实重现，但三次裸 `yield_now` 只解释约1 µs。两次 detached/oneshot handoff 与三次 deferred yield 的组合才重现大部分成本，不能把14 µs直接除以三，当成普遍的单次 yield 或 kevent 成本。

## 具体调用路径与证据边界

源码检查（Tokio1.48.0，mio1.2.4，macOS kqueue）：

1. `src/runtime.rs:351` 和 `src/interpreter.rs:206` 的 chunk 边界调用 `tokio::task::yield_now().await`，仍保留 default chunk=256。native 下一轮检查取消，tape 在 yield 后检查取消。
2. Tokio `src/task/yield_now.rs:57` 调用 `context::defer(cx.waker())`，第一次 poll 返回 Pending；`runtime/scheduler/defer.rs:32` 最终 wake 保存的 waker。
3. current-thread `CoreGuard::block_on` 在 root Pending 后执行普通 runnable tasks。队列空且 defer 非空时进入 `Context::park_yield`（`runtime/scheduler/current_thread/mod.rs`，搜索 `context.defer.is_empty`）；它先 `driver.park_timeout(Duration::ZERO)`，再 `self.defer.wake()`。`event_interval` 控制有 runnable 工作时周期性 driver turn，不能阻止此队列空路径。
4. 本例 root waker 的 `Handle::wake_by_ref` 设置 woken 并 `driver.unpark()`。detached Provider 经 `tokio::spawn` 执行后，oneshot 完成也唤醒 root。MiniPy 路径见 `runtime::receive` 和 native/tape 的 Provider spawn，不是同步直接返回 provider 的假执行。
5. time driver/driver stack 将 park_timeout 传到 I/O driver；`runtime/io/driver.rs:188` 调用 Mio `poll.poll(events,max_wait)`。mio `sys/unix/selector/kqueue.rs:95` 的 select 调 `kevent`；root unpark 经 `sys/unix/waker/selector.rs:26` 到同文件 `Selector::wake:253`，用 EVFILT_USER/EV_ADD/EV_ONESHOT/NOTE_TRIGGER 调 kevent。time-only 控制改走内存/thread parker，不经过该 I/O stack。

以上是固定依赖的源码调用链；下表是实测。没有用 syscall tracer 或内核采样，因此不声称每个 kevent 的调用数、耗时占比，或组合放大来自某个已证明的内核内部机制。尤其不能将14 µs全部归给空 poll 系统调用。性能成本与这些调度/唤醒路径组合一致，内核级细分未测。未修改 registry/依赖或系统设置。

三独立进程对、每进程每单元三批、每批1000次。数字为进程批均值的中位数，µs/op。无 MiniPy 编译、JSON 或网络流量，专门隔离组合：

| detached/oneshot handoffs | fairness yields | I/O-enabled | time-only |
|---:|---:|---:|---:|
| 0 | 0 | 0.002 | 0.002 |
| 0 | 3 | 1.272 | 0.262 |
| 2 | 0 | 0.981 | 0.366 |
| 2 | 3 | 14.734 | 0.623 |

空单元只作循环成本控制，没有 black_box，不代表可单独使用的 executor 基准。非空单元真实 await/spawn/oneshot。两次 handoff 分别在三次 yield 前后。其成本明显非加性，这是保留此隔离证据的原因。

## 唯一候选与停止标准

简单替代是保持默认61。提高 event_interval 的假设是：混合 ready runnable 工作足够多时，降低周期 driver poll 频率可能摊薄成本。源码已提示 idle/deferred 路径不受它控制，所以只做一次小实验，不实现新 scheduler。Host 可通过已有 Builder API试验，不需要在生产库加分支或参数。候选必须在真实 HTTP 混合、short sibling 和 prepare+execute 上有净收益，不能只看 ready scan。两侧都启用 I/O，没有拿 time-only 充当优化。

四对独立进程 AB/BA/BA/AB，同一 timing executable，3批/单元。canonical final 数据：

| workload/backend | lifecycle | default61 µs | interval256 µs | 变化 |
|---|---|---:|---:|---:|
| weighted/native | prepared | 16.105 | 16.071 | -0.21% |
| weighted/native | prepare+execute | 153.397 | 157.905 | +2.94% |
| weighted/tape | prepared | 21.619 | 21.507 | -0.52% |
| weighted/tape | prepare+execute | 39.217 | 39.187 | -0.08% |
| mixed/native | prepared | 80.415 | 80.325 | -0.11% |
| mixed/native | prepare+execute | 390.423 | 385.118 | -1.36% |
| mixed/tape | prepared | 108.146 | 104.569 | -3.31% |
| mixed/tape | prepare+execute | 121.888 | 121.941 | +0.04% |

Mixed tape prepared 四对变化 -17.00/+5.78/+13.69/-10.19%，不稳定。Mixed native cold 四对均略快，但旧 native scan cold 回退，prepared 几乎不变，不能据此采用 Host 调参。Scalar prepared native/tape default 为0.122/0.135 µs，candidate0.121/0.138；cold62.386/8.053→64.328/7.989 µs。time-only 的 prepared weighted native/tape1.798/7.670 µs，仅为机制控制，不能提供实际 HTTP。

首次 discovery 进程存在明显整体慢启动/交互噪声，所有原始数据照保留。发现裸 yield 不足以解释旧scan差值后，只增加两个无业务隔离单元并跑最终同样小配对，再以单独四格 wake probe分离 handoff×yield；不是为筛掉负结果反复调参。discovery mixed native prepared80.683→80.326 µs、cold384.695→385.119 µs，同样不支持采用候选。不做统计显著性、p95/p99、吞吐或普适常数声明。

## 工作负载、计时与正确性

`examples/al_io_probe.rs` / `examples/al_io_probe/support.rs` 使用现有 scalar/weighted Python fixture、真实 compiler、native/tape、ready Quote Provider。旧程序每次1000行、默认chunk256，即三次中间 yield。prepare+execute 每次重新构造 unit，包含准备、执行、输出校验/消费和清理；prepared 排除unit准备。Host/runtime/fixture/compilerTLS已热，不是进程冷启动。和旧fastpath_probe不同，本轮每进程用一个长寿命 block_on 包住批次；不把跨报告绝对数差当优化。

Mixed 用默认轮转 `tokio::join!` 并发执行一个weighted scan、一个scalar短任务和一条实际HTTP fetch→JSON i64叶乘法→publish→print的完整脚本。native/tape显式选择。cold三条分支均在计时内准备。短任务从三条分支共同进入 join 前开始计时，包含先被其他分支占用的时间；不是单独服务时间，也没有跨请求排队或开放到达负载。

短 sibling 完成时间，进程均值中位数µs：

| mixed lifecycle | native default→candidate | tape default→candidate |
|---|---:|---:|
| prepared | 0.644→0.636 | 0.596→0.585 |
| prepare+execute | 212.688→208.645 | 27.227→27.478 |

这些只是批平均 latency，没有逐请求分布。同步compile会阻塞短任务，这个成本没有移到计时外。

HTTP为同runtime localhost TCP fixture，共享HttpService/连接池。每条mixed请求真实进行两个HTTP往返。每批一次warm-up建池在计时外；prepared100次、cold20次，每单元3批。校验HTTP参数、`27\n`、scan/short输出，response delta严格为2×(n+1)，归还全部2个permits，进程仅一条连接。服务端构建在计时外，其响应工作在墙钟内。不测远程网络、TLS、业务API容量。循环等待仍真实发生，没有硬编码内核代替编译。

## 验证、身份与资源

本机 Apple M4/macOS26.4.1、rustc1.93.1；offline/locked、单编译job、唯一既有 `/Users/yuzhe/projects/minipy-native/target`，无下载/VM/WSL/Docker/其他并发性能任务。验证全部完成后可用磁盘约12→11 GiB，shared target约12→13 GiB，增量包括本工作区debug/all-feature测试和Clippy产物，未复制target。release两条probe可执行文件分别约9.8 MB/0.69 MB（Cargo同内容带hash文件不作第二套证据保存），不进Git；三个evidence目录合计约100 KB，不测RSS/PSS或density，不宣称内存改善。

`evidence/al-io-discovery` 420 batch rows/13 cells，`evidence/al-io`492/15，`evidence/al-wake`72/4，共984 batch rows。这不是984逐请求样本。全部进程保留，不选择较快进程。identity记录实际binary SHA256、probe source SHA256、lock SHA256、基线提交和工具链。discovery源码快照保留在`support.rs.txt`，hash与其identity一致。候选仅运行参数，两个arm完全相同可执行文件。raw CSV为唯一原始数据，summary只存聚合。

验证：macOS all-features89 tests、strict all-target/all-feature Clippy、fmt通过。生产src无改动，依赖无改动，沿用已验收AJ的默认/Wasm/Linux边界；本轮没有重跑Linux或Wasm，不作跨平台调度性能推论。取消/预算/owner/HTTP错误与late completion测试包含于all-feature执行，未削弱任何测试。

复跑（性能批次串行，不与构建/其他benchmark并发）：

    export CARGO_TARGET_DIR=/Users/yuzhe/projects/minipy-native/target CARGO_BUILD_JOBS=1
    cargo build --offline --locked --release --features tool-json --example al_io_probe --example al_wake_probe
    python3 tools/al_pairs.py "$CARGO_TARGET_DIR/release/examples/al_io_probe" evidence/al-io
    python3 tools/al_wake.py "$CARGO_TARGET_DIR/release/examples/al_wake_probe" evidence/al-wake
    python3 tools/al_pairs.py unused evidence/al-io --verify
    python3 tools/al_pairs.py unused evidence/al-io-discovery --verify --discovery
    python3 tools/al_wake.py unused evidence/al-wake --verify
    cargo test --offline --locked --all-features
    cargo clippy --offline --locked --all-features --all-targets -- -D warnings
    cargo fmt --check

停在独立签名实验分支交付，不merge main、不发布网站、不改冻结benchmark/AF/Pysolate，不继续追第二候选。

定向审查状态：固定集成提交2617a94已尝试通过Codex CLI0.146.0以`gpt-6-luna`、read-only启动，账户明确返回不支持该模型（`evidence/al-io/review-launch-failed.txt`）。这次启动失败不是审查通过，也没有换模型。交付分支后由独立看板review卡显式固定`gpt-6-luna`/`openai-codex`走Hermes路径执行，最终批准待该卡结果；若该路径亦不支持则应能力阻塞，不做静默替换。
