# K：最后一轮低成本热点优化

## 结论

撤回唯一候选：把解释器 `Next` 的 scan-header 读取后移到 chunk 边界。保留新增边界差分测试和全部证据，不增加第二项优化。最终 `src/` 与 b76be2dc804c54aa2725d45e403906bdfc0d263a 完全相同；现有 leaf 内联继续保留，默认 chunk=256、显式 backend 和语言/安全契约不变。

真实 Linux ARM64 四组 AB/BA/BA/AB，共 20,560 条 raw。冷 light CPU 汇总回退 1.76–2.06%，冷 scan 改善 1.69–2.66%，这四个 cell 的配对方向混杂。scan warm 改善 6.65%，但 100k stateful cold/warm 仅改善 1.09%/0.65%，配对方向也混杂。删掉的工作真实存在，但一次性源码全路径收益不足，不能只凭最好的 warm cell 保留。没有为追求正结果改权重、删异常样本或补跑第二轮。

后继 L 在本卡已发布、远端回读的精确交接 SHA 上做全面四路 benchmark 与图。本卡只有候选 qualification，不提供最终四路排名、新内存密度或持续容量结论。

## 定位与替换契约

基线仍有递归表达式求值和 flat statement dispatch。独立无插桩 macOS release sample：`stateful warm 100000 8000`，采样 3 秒，实际每 1 ms；eval 及包含解释器 run 的 `Prepared::run` 栈占主要执行样本。启动/dyld 样本也保留。该辅助 profile 不是 Linux 分项 CPU 比例，inlined run 不足以把开销分到各 opcode。既有 native 的 cold 编译和队列成本仍在 A–J 的历史证据中，本轮没有重新定位 native cycles。

本轮选择更小、可证伪的 dispatch 重复读取：原 `Next` 每行更新 index/chunk/pc 后，先读取 `unit.ops[header]`、检查 bounds 和 Scan tag，再判断 `chunk_left == 0`。ARM64 macOS 无插桩 release 汇编确认 header bounds/tag load 在 chunk `cbnz` 前，LLVM 没有自动把它移到边界路径。100k Next 与 391 次 fuel reservation 的数量级差给出了可删除工作的上限，不代表 CPU 占比。

候选在设置下一 pc 后，`chunk_left != 0` 直接进入下一 dispatch；仅在既有 chunk 边界读取 header、owner 长度并决定 yield。替换旧的每行读取，无额外字段、分配、prepare pass、哈希、fusion 或 fixture 特化。没有双执行与新 fallback。header 被实际读取时仍有空间边界/tag 检查。sealed flatten 保证每个 Next 指向 Scan，未公开可伪造的 opcode。取消、fuel reservation、owner release、行访问边界、checked arithmetic、短路和 Tool/输出/错误顺序均保留。

单独 Linux feature-gated diagnostic：weighted/stateful 100k 的 `next_header_lookups` 都由 100000 降到 391；原 leaf/expr/op 计数、fuel=391、yield=390 和 stdout 完全相同。计数二进制与速度二进制分开，计数不是时间份额。新增 counter 已随候选撤回，最终生产和既有诊断代码均不变。候选及计数 patch 保存于 `bench/results/k-final-hotspot/candidate.patch`。

## 有界测量合同

- 基线 b76be2d，候选只改上述运行位置。现有 Alpine ARM64 VM 从 off 独占启动，2 vCPU / 2048 MiB，Linux 6.18.52；首次 available 1867 MiB、swap use 0、guest disk available 11.1 GiB。无 Docker、CI、付费资源、系统/内核改动或其他 guest 实验。结束恢复 off。
- core Rust 1.93.1；benchmark Rust 1.96.0、Zig 0.15.2。复用已有 target，不复制缓存。default release 速度构建没有 trace、计数或 counting allocator；公共 Rust bench harness 未改。
- 沿用 H/J 五个 shape：scalar、weighted 100、stateful 100、weighted 10k、stateful 100k。light 权重 24/8/4/2/2，scan 8/16/8/4/4，synthetic 分布，非生产频率。
- 每 cell 四个配对，AB/BA/BA/AB；mixed 每 block 九个有限窗口，保留 sample>=2，全部 raw 留存。每窗口 40 个计划到达，500 µs 间隔，100 ms deadline 并排空。CPU 为保留窗口总进程 CPU/完成数，不含 sleep-to-deadline，包含 source clone、parse/prepare、Provider、执行与 cold unit 回收；planned latency 含队列/定时器。默认 chunk=256。ready 仍真实 spawn/oneshot，delay=1000 是两个真实 Tool sleep。
- standalone 每 shape/cold或warm 每配对 41 次，保留 sample>=11，120 个样本/版本/cell。cold 包含每次 parse/prepare、Provider setup、执行和 unit 回收；warm 是显式 prepare 复用，不声称 cache hit。预先构造源码、expected 和 logical prepared_bytes 诊断在区间外，两版本相同。standalone correctness assertion 在计时后；mixed assertion 在相同计时位置。stdout/result 完成后返回的 String 外层析构不计入 standalone 区间，未改变既有边界。
- 短任务为 scalar 和 100-row scans，p95/p99 为 pooled nearest rank。零 raw overload window；没有删除窗口。CPU 主指标，不把 µs 的壁钟或 Tool 等待当 CPU。小 shape 的调度/频率变异很大，逐轮值完整保留，不构造置信区间。

## Mixed 结果

单位 µs；相对变化为 candidate/base−1。尾延迟与 CPU 不保证同向。

| mix/mode/delay µs | CPU base→candidate | change | mean latency | short p95 | short p99 |
|---|---:|---:|---:|---:|---:|
| light/cold/0 | 513.57→522.60 | +1.76% | 1983.56→1975.22 | 2076.47→1955.28 | 2810.14→2506.63 |
| light/cold/1000 | 524.29→535.12 | +2.06% | 3694.12→3619.72 | 6932.82→6639.31 | 8107.10→8247.00 |
| light/warm/0 | 485.80→468.45 | −3.57% | 1713.72→1694.91 | 1566.76→1576.88 | 2072.07→2171.34 |
| scan/cold/0 | 799.97→778.67 | −2.66% | 4094.56→3929.20 | 3001.42→2602.49 | 5160.11→3928.97 |
| scan/cold/1000 | 821.23→807.38 | −1.69% | 7476.86→7661.53 | 7791.28→8234.69 | 8617.85→9036.24 |
| scan/warm/0 | 767.52→716.45 | −6.65% | 3446.22→3204.74 | 1981.09→1927.03 | 2535.26→2653.33 |

cold light 两个 cell 均两配对上升、两配对下降；cold scan ready 三降一升、delayed 两降两升；scan warm 四配对下降。delayed scan 短 p99 的四个配对 base 为 8579.21/8287.11/8179.40/8880.83，candidate 为 8993.21/8832.55/8806.33/9278.34，四个均上升，pooled +4.85%。这是保留的局部尾部风险，不能宣称全部由噪声导致，也不能凭四配对建立普适因果。scan ready 的大幅 pooled p99 下降包含 baseline 第二配对 7000.92 的突出值，不将它当稳定优化倍数。

## Standalone 全进程 CPU

单位 µs，四配对均值；prepare/execute 壁钟和逐轮 CPU 另见 summary，不能把这些壁钟分项当 CPU。

| shape | cold base→candidate | warm base→candidate |
|---|---:|---:|
| scalar 0 | 62.675→61.775 | 9.158→9.183 |
| weighted 100 | 115.308→119.917 | 32.525→23.417 |
| stateful 100 | 129.608→114.808 | 22.000→22.367 |
| weighted 10k | 621.500→605.308 | 527.600→424.433 |
| stateful 100k | 2415.125→2388.708 | 2365.025→2349.558 |

小输入与 weighted warm 汇总有显著 block variability，例如 weighted 10k warm 的逐配对变化 −6.08/+1.51/+7.72/−54.70%，不将 −19.55% 汇总宣称为稳定收益。stateful 100k cold 两降两升、warm 两降两升。源码/prepare 结构没有变化；观测到的 prepare 波动不能归因于本候选新增工作。

## 验证、冻结与复现

候选 macOS 和实际 Linux interpreter 各 11 tests 通过。新增差分测试针对 chunk 64/256/4096，各零行、1行、chunk−1/chunk/chunk+1、2chunk/2chunk+1，以及 fuel 精确足够/差1，比较 native/IR outcome、stdout、Tool 次数和 trace 的 phase/index/remaining/owner bytes。既有 cancellation、多 scan 累计 fuel、短路、错误输出、权限、receiver drop 与 late completion tests 保留。

最终撤回后 macOS all-feature release 和实际 Linux fixture-reference 各 62 tests 通过；Linux executable 上传后 SHA256 回读，再串行执行，不仅 cross-build。core strict all-target/all-feature clippy、core/bench fmt、diff check 通过。最初新测试的整数类型歧义导致一次编译失败，显式 usize 修正后通过，未改变验收。Rust benchmark 源码未修改。

最终 speed 重建与实测 baseline byte-identical，SHA256 `e1065d30d3a4238fe0560dc3165c1a9a75d1715840859d475d36565c47295c6f`，也与 J 的已保留 leaf 二进制相同。identity 保留源文件/二进制 hashes、命令、patch、toolchain、guest pre/postflight。最终没有新的 runtime/prepared 数据结构和 retained code，不借用 J 的内存值冒充 K 实测。

命令与结果在 `bench/results/k-final-hotspot/commands.txt`。关键入口：

```sh
cargo +1.96.0 zigbuild --manifest-path bench/Cargo.toml --release --target aarch64-unknown-linux-musl --bin interpreter_ab
python3 tools/final_hotspot_ab.py HOST KEY KNOWN_HOSTS BASE CANDIDATE
python3 tools/summarize_final_hotspot.py
cargo test --release --all-features
cargo clippy --all-targets --all-features -- -D warnings
```

A/B 脚本拒绝覆盖 raw。候选 patch 可用于诊断/速度重建，default 速度不会启用 profile；不得将候选用于 L 最终 campaign。精确发布 SHA 与 focused review 身份/结果在 K Kanban handoff。

## 停止点

局部 header 读取不是当前平均成本的足够杠杆。递归 eval/dispatch 与 frontend preparation 仍值得定位，但进一步寄存器/融合表示需改变准备/状态布局并重新算成本，本轮不实现。native 冷编译共队列隔离涉及 CodeUnit/owner 与跨线程契约，不通过添加线程框架解决。无自动 JIT、OSR、缓存、新 backend、JSON/字符串支持或第三轮。进入已安排的最终 benchmark。
