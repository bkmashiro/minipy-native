# J：有界热点迭代

## 一屏结论

保留 I 的叶操作数内联，只改解释器表达式求值；撤回 row-present 检查裁剪。没有新增自动 backend、缓存、混合 JIT 或默认 chunk 改动。当前 CLI 仍 native，解释器仍由调用者显式选择。

在现有独占 ARM64 Linux VM 上，两轮机制实验加一次最终混合回归共 58,128 条 raw。默认 chunk=256 的最终六个 CPU 单元下降 6.66–20.48%，24 个配对方向一致；这支持平均 CPU 的局部改进，不保证所有 workload 或小 shape 都变快。长 stateful 的 cold/warm CPU 下降约 19%/14%，weighted 10k 的单次采样则无稳定收益。

尾部是实际取舍：最终 scan 延迟 Tool 的短任务 p99 8334.73→9117.12 µs，四个配对均上升；scan warm 汇总 +11.92%，三个配对上升。平均延迟在前者也 +2.23%。ready 的 I 轮连续 p99 上升未稳定复现，不能宣称完全修复 tail，也不能证明变化只来自噪声。按用户授权的平均成本目标保留 28 行通用 leaf 快路，不将其宣传为 tail 优化。严格短任务 SLO 的调用者仍需按自己的 ready/延迟/复用分布验收；不新增人为 engine 阈值或 runtime 检查。

I 的双配置循环实际上三次均 baseline→candidate，未真正交替。J 使用 AB/BA/BA/AB，保留原结果并降低其因果归因强度。chunk=64 scan 的尾部敏感性保留，未为优化 CPU 改掉默认 chunk 或取消粒度。

相关 macOS all-feature 与实际 Linux fixture-reference 各 61 项测试通过。默认速度二进制无 diagnostic counters/allocator；最终重建 SHA256 与实测 leaf 二进制完全相同。VM 已恢复 off。原始证据：`bench/results/j-hotspots/`；重算：`python3 tools/summarize_hotspots.py`。

## 边界与方法

基线 b8bee915f17cc0db02dcdb068ddc89e8b0764c04。既有 Alpine ARM64 VM：2 vCPU、2048 MiB；初始 off，独占启动。预检 available 1858 MiB、swap 0、guest 可用 11.3 GiB。核心 Rust 1.93.1；bench Rust 1.96.0、Zig 0.15.2。无 Docker、CI、付费或全局配置改动。第一次无 PTY 的 vfkit 启动因 stdio device 不支持失败，改用 PTY 成功；非性能样本。

沿用 H/I 的 synthetic light 权重 24/8/4/2/2、scan 8/16/8/4/4，五个 shape 为 scalar、weighted 100、stateful 100、weighted 10k、stateful 100k。每有限窗口 40 个 planned arrivals，500 µs 间隔，100 ms 窗口并排空；非生产分布、非持续容量。current-thread 中冷编译与执行仍共队列。ready 仍真实 spawn/oneshot，delay=1000 是两个真实 Tool sleeps。

每机制四个 A/B 配对，顺序 AB/BA/BA/AB；每 cell 9 窗口，丢弃前 2 个但保留全部 raw。每 shape cold/warm 41 次，统计后 30 次；shape 是单个 candidate→base block，不是四配对，冷准备及 tiny 数值明显受调度/频率影响，不能作收益倍数依据。总进程 CPU/完成任务使用所有保留窗口 CPU 除任务数；短任务为 scalar 与 100-row scans，p95/p99 nearest rank pooled。窗口 CPU 不含 sleep-to-deadline，但包含排队、准备、Provider 和回收；计划到达延迟含 timer/队列。warm 显式复用，不假设自动 cache hit。stdout assertion 在 mixed 同一位置，JSON/expected 构造在计时外。

J 的 harness 仅新增一次启动的 MINIPY_CHUNK 配置，所有本项目 engine 经相同 OnceLock 读取 ScriptLimits；准备和执行成本没有隐藏。普通 Monty 保留原执行方式，没有声称其 chunk 契约等价。来源核对：相对基线只有 interpreter、diagnostic Counts、公共 bench harness 改变，其余 core/fixtures/provider/codegen 未变。

## 轮 1：leaf 内联

替换契约：在 sealed typed 表达式的 Int/Bool/Local/Field 子操作数上直接取值，删除递归 eval 入口；复合表达式仍恰好一次递归，保持左到右、短路和 checked i64。旧 eval 不在 leaf 成功路径运行；无 eligibility 遍历、重新执行、额外分配、准备遍历或永久双执行。新增一个内联 match，并有代码体积成本。Linux benchmark 文件 13,461,688→13,462,232 B（实际 identity 为准；不是 resident code 或内存密度测量）。

`leaf-chunks` 17,936 条，chunk 64/256/1024，两个 ready cold mixes。chunk=256 light CPU 591.73→546.50 µs（−7.64%）、scan 876.41→859.18（−1.97%）。light 四配对均改善，scan 两配对改善两配对小回退。对应 ready pooled p99 2956.71→2759.80、3929.44→3258.63；没有连续同方向退化。chunk=64 scan p99 2721.92→3572.51，前三配对上升；chunk=1024 light p99 +4.96%，scan −32.54%。这些结果说明 tail 与 chunk/ready 队列条件有关，不建立精确因果机制，不据此调整公共默认值。

单独 Linux diagnostic 执行见 counts.txt：weighted/stateful leaf_operands 520172/427064，剩余 recursive entries 360092/413617（对应 I 基线 880264/840681）。statement、fuel=391、yield=390 和输出不变。计数证明删除的工作，不是 CPU 占比。现有 differential tests 比较 native/IR phase、index、remaining、owner bytes、错误和 Tool 次数。未增加低扰动 queue trace，不能归因 timer/ready 队列中具体的一项成本。

## 轮 2：row-present check 裁剪，撤回

在 leaf 基础上将 row Option<Quote> 改为 owned Quote。假设 sealed 前端已证明字段只在 scan 内可读，每行复制的 Option tag/每字段 expect 可删；spatial bounds、owner 与预算均不变。准备无新 pass；用原生/解释器差分与 12 项相关测试确认候选语义。

`row-regression` 20,096 条，基线是 leaf，candidate 是 leaf+row；这是一次独立机制，不拿它与旧基线混算。light ready/delayed/warm CPU +1.79/+3.57/+3.28%，scan ready +2.94%；scan delayed/warm 汇总 −12.57/−2.79%，但 delayed 第二配对 candidate 327.96 µs 是突出异常，其余配对混杂。没有稳定平均收益，撤回。保存 `row-plus-leaf.patch`（对原始 b8bee915 的完整候选），`leaf.patch` 是已保留对照。最终 row 仍 Option，检查保留。

## 最终代表性回归

`leaf-final` 20,096 条，base 是原始 IR，candidate 是仅 leaf，默认 chunk=256。下面单位 µs，CPU 为全进程/任务；所有 24 paired CPU 比较下降。

| mix/mode/delay µs | CPU base→leaf | change | mean planned latency | short p95 | short p99 |
|---|---:|---:|---:|---:|---:|
| light/mixed-cold/0 | 588.65 → 523.67 | -11.04% | 2127.72 → 2000.63 | 1897.57 → 2064.38 | 2921.07 → 2611.02 |
| light/mixed-cold/1000 | 604.78 → 535.02 | -11.54% | 3883.40 → 3748.38 | 7119.98 → 7006.18 | 8274.31 → 8123.36 |
| light/mixed-warm/0 | 569.41 → 452.82 | -20.48% | 1982.34 → 1655.59 | 1654.52 → 1678.55 | 2123.57 → 2210.38 |
| scan/mixed-cold/0 | 909.89 → 791.40 | -13.02% | 4582.36 → 4010.54 | 2631.40 → 2675.78 | 3468.01 → 3558.35 |
| scan/mixed-cold/1000 | 895.03 → 835.41 | -6.66% | 7639.83 → 7810.32 | 7335.95 → 8208.64 | 8334.73 → 9117.12 |
| scan/mixed-warm/0 | 860.99 → 737.74 | -14.32% | 3955.24 → 3268.99 | 1948.96 → 1966.39 | 2396.85 → 2682.59 |

scan delayed p99 四配对 base 为 7970.95/7614.51/8371.56/8458.36，leaf 8682.63/8944.61/9453.40/9117.12。真实 repeatable 风险保留，且不把 sleep 算为 CPU 优化；不能由四配对证明普适因果。当前采用是平均成本与局部尾部之间的显式工程取舍，不是所有 cell 改善门。没有通过削弱 cancellation/fuel/权限、放大 chunk、额外策略或负载权重重写处理。

同次辅助 ordinary Monty/native/固定 Rust reference cold ready 各只跑一个 block，非四配对优化比较。light CPU leaf/native/reference/Monty 523.67/678.79/157.03/1844.32；scan 791.40/853.89/276.47/3817.74。reference 不付源码成本，不能用作未知源码 backend 的目标下界。全部 raw 的 overload：leaf-chunks 0，row-regression 与 leaf-final 各 10，仅 ordinary Monty scan。未删除超载窗口或异常样本。

本轮没有 frame/owner/prepared 数据结构变化、没有新缓存或 code retention。二进制体积成本见 identity；I 等待分配/RSS/PSS 是历史测量，不作为 J 新内存实测或容量结论。没有重新执行全 memory campaign。

## 轮 3 停止于成本判断

不实施混合 scan JIT。本轮已识别并保留便宜、高覆盖 leaf 收益，第二候选无稳定增益。现有 native 单位拥有整程序 frame/PC、liveness、Tool continuation，IR 使用 locals/owners/row/pc 分离状态；把 loop 单独交给 native 要新增边界 ABI、typed locals live-in/live-out、累积 fuel 和 owner 转移，以及禁止 Tool replay 的状态协议。当前没有可复用的 scan-only unit 构造入口，也没有真实编译成本证明会更便宜。仅凭长 I/O 不会摊薄 CPU。更大 frame/IR 改造超出本轮“小原型”的净价值，保留解释/native/reference；不建立自动 tiering、线程池或后台缓存，也不硬造编译成本上界数字。

## 验证与复现

- raw gzip、每次命令、source/toolchain/binary hashes、前后资源状态、patch 和 summary 同目录保留。总数由 Python 校验为 58,128。
- Linux tests：`cargo zigbuild --release --features fixture-reference --target aarch64-unknown-linux-musl --tests --message-format=json`；从 compiler-artifact executable 逐个上传、SHA256 read-back，再实际串行 `--test-threads=1`。61 项通过，见 linux-tests.txt 与 linux-test-identity.json。
- macOS `cargo test --release --all-features` 61 项通过，见 macos-tests.txt。core strict all-target/all-feature clippy、两个 fmt check 通过；bench clippy 仅沿用两项 intentional lint 例外。初次 bench 使用默认 Rust1.93 被版本门拒绝，改用1.96通过，保留失败日志。
- diagnostic `cargo zigbuild --release --features interpreter-profile --target aarch64-unknown-linux-musl --example interpreter_counts`，单独上传运行 weighted/stateful 256，hash 读回与本地相符。
- 速度构建 `cargo +1.96.0 zigbuild --manifest-path bench/Cargo.toml --release --target aarch64-unknown-linux-musl --bin interpreter_ab`，保存 baseline/leaf/row 三个 executable，不复制 build tree。最终重建与 leaf 实测 hash 完全相同。
- `python3 tools/hotspot_ab.py HOST KEY KNOWN LABEL BASE CAND chunks|regression`；LABEL 必须新建，脚本拒绝覆盖 raw。`python3 tools/summarize_hotspots.py` 重算本轮全部汇总。
- vfkit 仅在 PTY 中启动，结束 guest poweroff 与 vfkit exit0，恢复初始 off。未新增依赖/CI，target 不提交。最终固定快照独立审查结果和发布 SHA 在任务交接记录。
