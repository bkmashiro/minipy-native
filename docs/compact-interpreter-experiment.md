# N：紧凑计算表示与前端修正

## 结论与版本

保留提案 1 的三项修改：32 槽 definite-assignment bitset、直接写入已有 statement 容器，以及解释器 computation tape。前两项分别提交为 `a4d68c1` / `81d5df0`，第三项直接替换原递归表达式执行。没有改 native backend、调度契约、Provider 配额、默认 chunk 或语言/安全语义。

同轮实际 Linux ARM64 四组 AB/BA/BA/AB，20,560 条速度 raw；六个 mixed CPU 单元全部四配对下降，冷路径汇总改善 23.40–46.04%。100k stateful 冷路径/显式复用 CPU 改善 48.29%/53.09%。小输入收益不一致，weighted 100 cold 回退 15.82%，部分短任务尾延迟上升，完整保留。选择保留依据是一次性源码 mixed 全成本的方向一致与大扫描收益，不是宣称每个程序都更快。

基线是 `839503936b4d1eeca892ebeeed49abcb378be4bf`，其 `src/`、core Cargo 与审查固定 `8a293ec` 的实际 diff 为空。L 已验收，本轮没有与 L 并写。L 全套四路/Python/汇编图仍是冻结旧版证据，本轮只提供新版与当前生产表示的配对 qualification，不冒用旧图证明新版排名。

## 替换契约

- 保留 RustPython parser、共享 `Program`/typed semantic IR、静态类型和 native F predicate lowering。前端 `initialized` 用 `u32`，schema 的 0/32 slots 用 u64 建 mask，避免移位 32；分支求交、循环恢复和输入只读检查不变。
- `compile_stmt` / legacy return 接收已有 `Vec<Stmt>`，删除每语句临时 Vec；一条源码语句仍可产生 Record+Print / Record+Return，多 statement block 仍有自身必要容器。
- 在既有 flatten 遍历中消费 boxed Expr，输出一个连续 `Vec<Op>`。Prepared unit 只持有 tape、既有 owner-liveness mask 和 input schema，不保留两份表达式表示，也不新增全局 optimizer pass。
- Operand 为立即数、静态 local、logical Field 或短命 Temp。保留 J 的叶操作数内联思想，不走递归 `eval` 或运行时名字查找。Field 仍是 schema 字段枚举，没有锁死成 JSON 裸 offset。
- Add/Sub/Mul 直接 checked 写目标；比较的条件消费者直接跳转；值消费者仍得到正确 Bool。`and`/`or`/`not` 通过短路跳转生成，左到右检查与错误位置不变。目标 local 在整个 RHS 读取后才写入，含自引用 bool 赋值。
- `run_until_boundary` 是同步函数，每 turn 有最多 MAX_NODES=512 个 i64 scratch 槽（4 KiB 栈上工程上界，来源于已有 frontend admission）。scratch 不在 `State` 或 async future 中，不跨 Tool/chunk 挂起。每语句重用槽编号，不做跨语句临时值复用、CSE、重关联或常量折叠。
- 原 Scan/Next、按 chunk fuel reservation、header 读取、owner release 与取消检查位置保留。K 的 header 后移候选未复活。Tool 参数在原授权前求值，checked overflow/先前 stdout/Provider effect 顺序不变。没有 unsafe scratch 或 runtime fallback。

新 tape 在 prepare 中增加显式指令与短路回填的暂存分配。没有假设 lowering 免费，费用均在 cold 区间中。旧生产 prepared expression evaluator 已删除，不保留双执行路线。

## 测量边界

沿用 H/J/K synthetic 五 shape：scalar 0、weighted 100、stateful 100、weighted 10k、stateful 100k。light 权重 24/8/4/2/2，scan 权重 8/16/8/4/4，非生产请求频率。每窗口计划 40 arrivals，间隔 500 µs，100 ms deadline 后排空。既有 current-thread Tokio harness 未修改。

- mixed 每 block 9 个窗口，保留 sample>=2；standalone 41 次，保留 sample>=11，四配对每版本/cell 120 个样本。全部 raw 包括 warmup/突出的慢样本，零 overload window。
- 全进程 CPU 为主指标。mixed 为保留窗口 process CPU/完成数，包含 timed source clone、parse/prepare、Provider、计算和 cold unit 回收，不含 sleep-to-deadline。planned latency 含 timer/排队。ready 仍真实 spawn+oneshot；delay=1000 是两个真实 Tool sleep。
- standalone cold 每任务重新 prepare，warm 显式 prepare 复用。预先生成 source/expected 与一次 prepared_bytes 诊断在区间外，两版本相同；Provider setup、执行、unit 回收在内。returned stdout String 的最外层析构仍不在 standalone 区间中。分项 prepare/execute 是壁钟，不能当 CPU 分解。
- default speed 没有计数或 counting allocator。waiting-memory 为独立构建、独立进程。既有 VM 从 off 独占启动，2 vCPU/2048 MiB，Linux ARM64 6.18.52。前置实测 available=1863 MiB、swap used=0、guest disk available=10.5 GiB，无并行 guest 实验、Docker、CI、付费资源或全局配置改动。
- toolchain 为 core Rust 1.93.1、bench Rust 1.96.0、Zig 0.15.2，target=aarch64-unknown-linux-musl。复用既有 target，不复制依赖缓存。

## Mixed 全进程 CPU 与短任务尾部

单位 µs，四配对汇总。CPU delta=candidate/base−1。

| mix/mode/delay | CPU base→tape | delta | short p95 base→tape | short p99 base→tape |
|---|---:|---:|---:|---:|
| light/cold/0 | 441.04→337.84 | −23.40% | 1925.98→1941.26 | 2727.74→2528.93 |
| light/cold/1000 | 540.14→346.68 | −35.82% | 6862.57→6712.98 | 8146.39→8059.43 |
| light/warm/0 | 485.42→355.55 | −26.75% | 1611.86→1760.41 | 2013.76→2129.81 |
| scan/cold/0 | 735.44→396.81 | −46.04% | 2480.29→2249.34 | 3514.15→3059.79 |
| scan/cold/1000 | 584.92→415.02 | −29.05% | 7579.07→7750.05 | 8725.72→9001.98 |
| scan/warm/0 | 525.88→357.25 | −32.06% | 1917.37→1804.99 | 2939.07→2187.76 |

方向一致不代表精确稳定比例。light delayed 第二配对 CPU 变化 −80.11%，scan ready 逐配对 −67.35/−41.53/−14.18/−65.46%，有显著 block variability。不是置信区间或持续服务吞吐证明。short nearest-rank p99，light warm 三配对上升、一个下降（−3.96/+20.71/+10.33/+0.21%）；scan delayed 三升一降（+18.89/+8.09/−3.19/+17.37%）。这些局部风险保留，未通过改变权重或删除样本掩盖。

## Standalone 与 preparation

单位 µs，全进程 CPU 均值。

| shape | cold base→tape | warm base→tape |
|---|---:|---:|
| scalar 0 | 48.97→45.46 | 4.18→7.16 |
| weighted 100 | 74.18→85.91 | 16.13→19.36 |
| stateful 100 | 85.58→88.28 | 11.72→12.09 |
| weighted 10k | 434.98→332.64 | 395.63→227.96 |
| stateful 100k | 2502.19→1294.00 | 2475.68→1161.32 |

小输入没有一律改善：scalar warm +71.12%，绝对 +2.98 µs；weighted 100 cold +11.73 µs，三配对上升一个下降。stateful 100 cold +3.15%，三降一升。大扫描八个 cold/warm 逐配对全部改善。100k stateful 多数行只执行 predicate，不能泛化成所有重算术任务的 crossover。

Linux cold prepare 壁钟 scalar 38.77→37.08、weighted 100 60.88→68.78、stateful 100 71.29→76.49、weighted 10k 52.46→67.49、stateful 100k 43.43→48.94 µs。Tape preparation 成本真实存在，大扫描执行支付了这部分；不从 warm 结果推定源码有缓存命中。

### 两项前端归因（辅助 macOS，不混入 Linux结论）

分别 baseline→bitset、bitset→输出容器，四 AB/BA/BA/AB，每 cell 121 样本保留>=21，共 11,616 条 raw。三小 shape 同时测解释器/native cold，记录 prepare wall 和全进程 CPU。数据由 `frontend-macos-identity.json` 精确绑定三个签名阶段及二进制。

| change / interpreter shape | prepare wall base→changed µs | full CPU base→changed µs |
|---|---:|---:|
| bitset / scalar | 8.33→11.85 | 9.26→13.21 |
| bitset / weighted 100 | 26.22→24.12 | 31.17→28.79 |
| bitset / stateful 100 | 27.17→26.01 | 31.11→29.85 |
| container / scalar | 8.66→11.99 | 9.64→13.41 |
| container / weighted 100 | 26.46→23.58 | 31.54→28.16 |
| container / stateful 100 | 25.96→25.26 | 30.11→29.21 |

native prepare dominated by unchanged Cranelift，bitset 三 cell 64.72→64.44、144.51→146.74、159.23→159.38；container 65.15→64.03、155.63→143.89、159.75→159.32 µs。这些辅助 small-shape 均值受运行状态影响，不证明每项都有稳定普适提升，更不声称 bitset 解决 native compilation 大头。保留它们的依据是删除现有有限域哈希和临时容器工作、保持语义，以及 integrated source-once mixed 的净收益；未据此替换 parser。

## Prepared 表示、等待内存与代码

Logical prepared_bytes 是 layout/capacity 诊断，非 RSS：scalar 770→594、weighted 1322→1090、stateful 1434→1114 bytes。扣除所有既有持久数组后没有隐藏 retained Expr，scratch 不在 prepared bytes 内，因为它是同步 turn 栈空间。

96 个实际 Linux waiting snapshots，两个相反 build 顺序，mixed 40 tasks/独有 unit，standalone 32 tasks/独有或显式 shared unit。四 phase 为 startup/load_wait/record_wait/reclaimed。counting allocator 的 requested live 增量扣除 payload、再除任务数，包含 Tokio、Provider、输出和 code，不是仅 State 大小。

| case/code | load overhead bytes/task base→tape | record overhead base→tape |
|---|---:|---:|
| light mix/unique | 2292.50→2186.50 | 2281.35→2175.35 |
| scan mix/unique | 3255.35→3043.35 | 3234.60→3022.60 |
| weighted 100/unique | 3220.25→2988.25 | 3188.31→2956.31 |
| stateful 100k/unique | 3332.25→3012.25 | 3330.31→3010.31 |
| weighted 100/shared | 1924.06→1916.81 | 1892.12→1884.88 |
| stateful 100k/shared | 1927.56→1917.56 | 1925.62→1915.62 |

减小幅度对应 prepared unit 的差值与显式共享分摊。没有每等待任务 +4096 bytes scratch；reclaimed requested 增量逐 cohort 相同。RSS/PSS 页粒度、allocator retention 与微小 stdout/log residue单独在 raw/summary 保留。例如 stateful unique load PSS 55,111,680→55,101,440，shared 55,054,336→55,054,336 bytes，不夸大成显著物理密度变化。此 raw 只给 sampled peak，非精确 worst-case peak。

含四路 harness/dependencies 的 ELF 文件大小 13,500,056→13,497,256 bytes，整体 `.text` 0x849948→0x849668（−736 bytes）。不是单独 interpreter 内核机器码大小，未重测 native JIT 代码变化。速度/内存二进制 SHA 与字节数见 identity。

## 可达服务限制：本轮只纠正文档

新增实际差分测试确认：一个 256-row buffer 上连续两次 scan、chunk=256、fuel=512，两 backend 均输出512且没有 yield。累计 fuel 正确，但 chunk 不是任务级时间片。

`runtime::receive` 取消后及时返回，detached spawn 和 Provider 参数继续存活，late result 不恢复旧任务。`tool_calls` 是任务局部，buffer cap 在 receive 后检查，runtime 没有全局在途 permit。持续准入/取消可能积累尚未完成的 Provider future/内存。这是源码可达的服务边界，不是本轮实测内存漏洞。全局 permit 与跨 scan turn allowance 只记录未来服务工作，没有偷偷改变调度/授权契约。

## 验证与复现

macOS all-feature release 和实际 Linux fixture-reference 各 66 tests 通过。新增 32槽/0槽初始化、自引用复杂计算/Bool 消费、短路运行期 overflow/先前输出、两 Tool/多 chunk scratch 重建、准确 chunk 非任务切片测试。既有差分同时比较 outcome、stdout、Tool counts、owner bytes、fuel/index/phase，以及取消/迟到完成/receiver drop/资源/权限边界。strict all-target/all-feature clippy、core/bench fmt、Python语法与 git diff check 通过。

独立 `interpreter-profile` Linux binary 确认旧 compound eval weighted/stateful=360086/413532；新表示无递归 Expr evaluator，分别 506803/413696 次 tape dispatch。历史 expr/ops/leaf fields 在 tape 路径为0，不能把字段0当作无需操作数读取。fuel=391、yield=390、stdout 完全相同，计数不是 CPU 占比。

执行中两项修正如实记录：

1. Speed 已收全，memory runner 把 mixed-hold 的 reps=40 当成任务数，实际生成40组 snapshots，assert 捕获。初始 memory 文件为空，速度 raw 未覆写或重跑；修正 reps=1 后只恢复 memory。speed preflight 未被原 runner checkpoint，恢复 identity 的 preflight 明确标作 memory-resume；独占启动前置实测另在任务执行记录。
2. 共用 target、切换历史源码目录后，首次新 diagnostic build 复用了旧 baseline executable。无有效新计数结论由该次产物产生。清理项目 package 的构建产物、重建并上传 SHA 回读后取得当前计数；同样强制重建 speed/allocator，均与速度/内存实测保存二进制 byte-identical，主性能 raw 不受影响。新增 chunk 文档测试有一次 helper 名字编译错误，改用现有 TestProvider 后通过，无验收削弱。

小 raw 与重算工具位于 `bench/results/n-compact/` / `tools/compact_*.py`。关键命令：

```sh
# 历史与当前源码使用同一个 target 时，切换前强制重建项目包，避免旧产物复用。
cargo clean -p minipy-native --manifest-path bench/Cargo.toml --release --target aarch64-unknown-linux-musl
cargo +1.96.0 zigbuild --release --locked --manifest-path bench/Cargo.toml --target aarch64-unknown-linux-musl --bin interpreter_ab
# 保存 speed，再单独开启 allocation-diagnostics 保存 memory binary。
python3 tools/compact_ab.py linux HOST KEY KNOWN_HOSTS
python3 tools/compact_ab.py attribution
python3 tools/compact_summary.py
cargo test --release --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo zigbuild --release --locked --features fixture-reference --target aarch64-unknown-linux-musl --tests --message-format=json > target/n-compact/linux-tests-build.json
python3 tools/compact_verify.py HOST KEY KNOWN_HOSTS
```

Runner 默认拒绝覆盖 raw；复现用新 result 目录。Baseline 使用8395039的源码与同版 harness，前端三个阶段在 identity 中，候选源 patch/hash 在 Linux identity。完整构建命令、最终 source identity、实际 Linux executable hashes/commands 和日志均另存。无新依赖或全套跨引擎 campaign。

停止在当前提案验收与一次固定快照 focused review，不实施提案2服务worker/permit、提案3scanJIT、自动策略/缓存、JSON/字符串或新backend。
