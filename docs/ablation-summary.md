# 消融结果与静态约束账本（截至 BH，2026-10-15）

## 一页阅读

本页汇总**已存在**的局部实验，不新增 benchmark，也不是这些实验的联合 A/B。时间变化统一按 `(候选耗时 / 基线耗时 − 1) × 100%`；负值为加快，正值为回退。单位、CPU/wall、source-cold/prepared、机器、样本及版本随行标注。不同机器、源码版本、输入和计时边界的百分比不能相加；速度倍数若需要，应另算 `基线/候选`。紧凑数据见 [`ablation-summary.json`](ablation-summary.json)，值是已有报告的摘要转录而非新测量。

目前可保留的代表：F 的 predicate-only native lowering 在旧版 ARM64 Linux 10k weighted/nested 冷路径约 −11.3%/−11.1%；J 的 leaf 快路在六个有限 mixed CPU 单元约 −6.7% 至 −20.5%，但 delayed scan 短 p99 +9.4%；N 的 sealed tape 在其基线的六个 mixed CPU 单元约 −23.4% 至 −46.0%，weighted 100 cold 则 +15.8%。这些是不同阶段的独立配对，不是同一最终版本的总提速。N 的前端 bitset/容器改动是一起保留的阶段，不应把 tape 的总体效果全部归因于它们。I 曾撤回 leaf；J 以真正 AB/BA/BA/AB 重测才保留。K 的 Scan/Next header 延后虽将 100k 次读取降至 391 次，cold light CPU 却回退，故撤回。旧 IR/M4 单遍 allocator 冷 +14.6%、prepared +111.5%，未在当前 CFG/Linux 复测，不应按诊断 regalloc 桶估计收益。

AH 的 opt-in 静态 Tool/JSON 带来能力，不是零成本开关：最初旧标量小批次显示回退，但相同调度配置的 AS 三对整合控制没有稳定重大旧路径回退；真实 I/O-enabled 调度在 ready 1000-row Quote 上仍额外约 14 µs。AY 的 opt-in 固定宽度 record 入口可运行，然而 Quote tape 旧路径在 BD 修复后相对 pre-record 仍冷 +4.7% / warm +17.0%；没有并入 main。BF 的 C1 节省等待态 39.5 allocator-requested B/task 而请求无稳定收益，C2 两轮局部改善但双执行体 +16,480 B、机制未隔离，均撤回。BE 以 JoinHandle 替换 oneshot 也未稳定降低 wall 或整进程 CPU。BG 的成功验证缓存已由 BH **单独**合入 main：自然 alias+单次 contains 的 16 KiB tape cold hit 三对 −15.5/−12.8/−12.9%，64 KiB miss 三对 −26.0/−25.4/−19.1%；短 warm native 三对反而 +3.2～5.9%。没有把 BG 三次 contains 的结果冒充单次使用收益。

强简单对照同样重要：AH 已驻留 1000-row 数组的 typed 解码约 33.5 µs，lazy row-slots 127.7 µs；早期 Linux 10k weighted 源码冷路径的有限 Host helper 83.3 µs，native 279.3 µs（能力/调度契约不同）。这阻止以“静态化”名义扩成通用数组 VM、任意 kernel 目录或默认 JIT。所有微秒结果都不是生产负载或服务容量承诺；请求错误顺序、权限、累计预算、取消和 owner 生命周期不得为数字牺牲。普通 user error 返回 task error；编译器、降级、runtime 和可信 provider 都在 TCB。eBPF 若作为前置验证的比喻，不表示当前代码运行于内核或隔离恶意原生机器码。

## 详细消融表

| 阶段/状态 | 边界与机器 | 原始基线 → 候选；时间变化 | 删除的工作、仍需做的工作及代价 | 原始依据 |
|---|---|---|---|---|
| E 诊断，仅观察 | Linux ARM64 VM，旧 CFG，fresh-source compile wall | scalar 227.46 µs，weighted 330.12 µs；未测 A/B | diagnostic define 占 82.1–85.8% 编译 wall；regalloc/verifier 是插桩 exclusive 桶，五次 verifier 未证明冗余，不能当可节省金额 | [E](compiler-profile.md#three-identified-backend-hotspots)，`bench/results/e-compiler-profile/` |
| F 保留 predicate | Linux ARM64，10k weighted/nested，cold source-to-result wall，`51e83bd`→F | weighted 324.437→287.923 µs (−11.25%)；nested 466.784→414.577 (−11.18%)；scalar 控制 130.797→130.881 (+0.06%) | `if` 条件按 short-circuit 直接分支，不物化 bool join；bool 值消费者仍物化；checked errors/Tool 次序不变，代码块/字节下降 | [F](compiler-profile.md#f-predicate-lowering-experiment-2026-10-14)，`bench/results/f-predicate/followup-summary.json` |
| 旧 allocator 撤回 | 旧 IR，macOS M4，10k weighted/chunk4096，ready provider，cold/prepared full wall | cold 117.604→134.7705 µs (+14.60%)；prepared 20.2295→42.792 (+111.53%) | single-pass 不是默认，当前 general CFG/Linux **未测**；不可用旧桶预测新收益 | [E 历史](compiler-profile.md#ranked-candidates-and-historical-negative-evidence)，`docs/experiments.md` E |
| J leaf 保留；I 曾撤回 | Linux ARM64，五形状 40-task 有限窗口，mixed CPU/task，默认 chunk256 | light cold ready 588.65→523.67 µs (−11.04%)；scan cold delayed 895.03→835.41 (−6.66%)；scan delayed short p99 wall 8334.73→9117.12 (+9.39%) | 叶操作数不再递归调用 eval；复合表达式、错误与短路保留；末尾上升四对一致，不能声称 tail 也快 | [J](hotspot-iterations.md#最终代表性回归)，`bench/results/j-hotspots/` |
| K Scan/Next 撤回 | Linux ARM64，同有限窗口 CPU/task | light cold ready 513.57→522.60 µs (+1.76%)；scan cold ready 799.97→778.67 (−2.66%)；scan delayed p99 8617.85→9036.24 (+4.85%) | header bounds/tag lookup 从 100000→391（独立 diagnostic），但冷平均净值不足且 tail 上升；安全检查仅在实际读取时执行，最终候选不存在 | [K](final-hotspot-round.md)，`bench/results/k-final-hotspot/` |
| N compact + frontend 保留 | Linux ARM64，40-task mixed CPU/task，旧 `8395039`→N | light cold ready 441.04→337.84 µs (−23.40%)；scan cold ready 735.44→396.81 (−46.04%)；weighted100 cold standalone CPU 74.18→85.91 (+15.81%) | `u32` 初始化 bitset、复用 statement Vec、一次 flatten 成 tape 去掉递归 Expr；scratch 栈 4 KiB/turn，prepare 可能更贵；不能分摊总体效果为三个独立收益 | [N](compact-interpreter-experiment.md)，`bench/results/n-compact/summary.json` |
| AH / AS opt-in 保留 | M4 time-only ready provider，旧标量与 weighted 1000；cold/prepared full wall | AH scalar tape cold 8.800→11.692 µs (+32.86%) 的首次小控制；AS 对齐 old scalar tape 9.842→9.473 (−3.75%)、weighted tape 29.135→28.410 (−2.49%)；同 AS I/O/time weighted native prepared 16.828/2.111 µs（不同 driver 控制） | 静态 Tool IDs/schema/路径不在每行动态发现，重复 JSON 路径缓存；native JSON 仍进共享 helper；opt-in tape +32 B header，I/O reactor+yield 未删；AH 与 AS 非同一 A/B | [AH](tool-json-results.md)，[AS](as-integration.md)，`evidence/as-integration/old-paths/summary.json` |
| AJ schema-clone 撤回 | M4，JSON cold wall，四对 | few native 88.789→90.489 µs (+1.91%)；repeated tape 76.957→78.987 (+2.64%) | 尝试少克隆注册 schema 却无净收益；当前仍在准备期复制所需声明；direct Tool print 是另一个功能变更，非这项提速 | [AJ](aj-bounded-results.md)，`evidence/aj-bounded/` |
| AY/BD 实验，未合 main | M4，ready 4096-row Quote，full-task wall，cold/prepared（warm=显式复用） | AY→BD A Quote tape warm 53.729→45.416 µs (−15.47%)；pre-record→BD A cold 54.875→57.480 (+4.75%)、warm 30.854→36.104 (+17.02%)；BD B 相对 A cold 76.229→102.416 (+34.36%) | AY record 固定 stride/field import 一次验证，仍需 row/fuel/bounds；BD A 合并 Quote row 借用恢复部分成本，B 拆字段 operand 撤回；AY/pre-record 布局机器码未测，残余不能归因为 State 尺寸 | `exp/bd-schema-cost:docs/experiments.md` AY/BD，`bench/results/bd-final-{ay,base}-ab-macos.jsonl`（实验分支） |
| BF C1/C2 撤回 | M4，ready Quote 4096，cold/prepared full wall | C1 warm 39.896→39.770 µs (−0.32%)；C2 第一次 cold 65.417→60.667 (−7.26%)、warm 40.270→35.416 (−12.05%)，第二轮各约 −8.0/−9.3% | C1 wait 16-task 减 632 requested B，future 未变；C2 多一个解释执行体 +16,480 B，动态改善原因未隔离，均未合主线 | `exp/bf-record-isolation:docs/experiments.md` BF，`bench/results/bf-*`（实验分支） |
| BE JoinHandle 撤回 | M4，I/O-enabled current-thread，mixed HTTP+scalar+scan full wall；全进程 CPU 独立跑 | mixed native prepared 93.590→93.643 µs (+0.06%)；tape prepared 102.603→108.820 (+6.06%)；进程 CPU 0.128288→0.128677 s (+0.30%) | 仅删完成通知 oneshot，不删 spawn/fair yield/IO driver/permit；约 13.43 µs 控制不可全部归因 channel | `exp/be-http-cost:docs/experiments.md` BE，`evidence/be-http-cost/{summary,process-cpu}.json`（实验分支） |
| BG/BH **仅 BG 合入** | M4，自然 alias+单 contains，预编码 Bytes ready，source-cold full wall，三对 AB/BA/AB | 16 KiB tape hit −15.48/−12.81/−12.93%；64 KiB miss −25.96/−25.41/−19.05%；短 warm native +3.21～5.86% | 首次 sonic 校验 marker 后借原 Bytes，无重复 parser 校验；仍按访问收费、检查类型/UTF-8/预算、escaped 一次解码；State 128 B/path 24 B 不变，wait RSS **未测** | [BG/BH](experiments.md#bh-selective-integration-decision-2026-10-15)，[BH](bh-stage-conclusion.md)，`evidence/bh-string-integration/summary.json` |

表中百分比按显示的四舍五入均值计算，可能与原报告未舍入的 pair 值末位不同；BG/BH 用原逐对百分比而非跨对均值。K、N 的 CPU 是有限 workload 的进程 CPU/完成任务，F/AH/AS/AY/BE/BG 的主表是 wall。`prepared` 仅显式复用同一单元，绝不代表现实自动缓存命中。AY/BD/BF/BE 的相对路径以所列实验分支为准，不能在 main 找不到就推断缺证。

## 静态化与费用：按生命周期看

| 阶段 | 已删的重复工作/结构 | 仍不可省的边界及新代价 |
|---|---|---|
| 编译器 AOT | 前端把名字、scalar slots、row field 和 Tool 调用绑定；F 的 predicate 分支不构造一次性 bool join，N 在 flatten 时产 tape 不保留递归 Expr。 | fresh-source parse/type/lower/codegen/install/cleanup 仍计入冷请求。Native Cranelift 的 verifier/寄存器分配、sealed CFG、resume PC 与代码内存有真实成本；无缓存命中假设。 |
| Tool 注册 | Host 先声明 ID、签名、参数和结果 schema；权限不需 per-field 动态授权；调用绑定可直接编成有限 Tool 入口。 | 声明输入在准备时校验；每任务 Tool 授权/调用预算和提交顺序、真实 provider 错误仍检查。AJ 克隆消融负结果说明“不再复制一点 schema”未必抵得过代码变化。 |
| 用户准备 | 有限 Python 子集拒绝未支持形状，初始化位图和静态字段路径替代动态名字/对象图；finite helper 可覆盖特定聚合。 | 用户源码每次新版本仍需解析/类型化；语义不等 Python 任意精度/全 JSON。新增 tape ops、native 编译、schema 描述与 code resident；完整源脚本不能换成手写固定业务 kernel 并称公平。 |
| 输入安装 | Quote 冻结复用 typed payload；JSON Bytes root/路径按需访问，重复路径缓存；AY 仅实验的 record 在 import 一次校验布局、bool 与上限。 | 外部字节 import、schema 和分配范围验证必需；借用必须保活 owner，escaped decode 和 root/decoded-capacity 计费；微小 view 可钉住大 root，安装与适配不能移出冷路径隐瞒。AY 的 record wire decode 也可能比现成 Quote 贵。 |
| 执行 | native 直接行循环不建每行对象；J 避免 leaf eval 递归；N tape 取代递归 Expr；BG 避免同串重复 sonic 校验。 | 保留 checked i64、bounds、per-task fuel/chunk、短路错误顺序、取消边界；native JSON 仍有 helper/cache/type/预算工作。K 的提取工作虽真实却非净收益，BF C2 以重复执行体换局部速度未过取舍门。 |
| 等待回收 | 最后一次使用前释放 Quote/root/decoded lease，任务取消后不被迟到 completion 复活；显式复用代码与独立 task state。 | Provider 已提交的 detached operation 需持参数/permit 至完成；oneshot/调度 wake 和 I/O driver 有成本，BE 换通知容器未消掉。无全局 in-flight permit，任务级预算不等于服务总容量；drop owner 不自动降 RSS。 |

## 下一步假设，不是已做的优化

1. AOT：在当前 native CFG 仅针对**确无 suspension/yield 的源码**审视 PC entry 的生成条件（见 [`src/codegen.rs`](../src/codegen.rs)、[E 排名](compiler-profile.md#ranked-candidates-and-historical-negative-evidence)）。F predicate 已做；no-suspension entry **未做**。需先证明不存在非 entry resume，保留故障/输出语义，再以冷请求及代码尺寸和 prepared 运行对照；不能把旧 allocator 重新包装成新机制。
2. 注册/准备：给 [`src/frontend.rs`](../src/frontend.rs) 的 Tool 声明验证、clone 和 slot binding 做阶段归因，确认实际未知 Tool 数/声明大小之前**不**恢复 AJ 的 schema-clone 候选。它已测为负；若发现能整段删除的重复验证，先证明静态不可变声明已验证，再以 fresh-source 全费用测，不预设收益。
3. 输入安装：用 [`src/json_view.rs`](../src/json_view.rs) 的路径缓存/owner 结构做实际读多字段、多次访问的代表性准入；BG 成功 marker **已做**，全数组 lazy row-slots **负结果**。只有真实 typed array/source 需求及 import+执行端到端控制改变时再评估 typed decode/row-scan，不能仅重跑相同 synthetic 数组把 AY 当收益。
4. 执行：先用 [`src/interpreter/tape.rs`](../src/interpreter/tape.rs) 和现有 raw 的真实 Quote 访问、sealed Op 结构定位 AY/BD 残余约 17% 的动态热块及 layout，不能仅以 `State=328 B` 推因；record branch **未合 main**，BF 双执行体 **已撤回**。仅在等价 record 实际需求和稳定旧路径门过后考虑单体静态分支删减，别复活 C2。
5. 等待回收：对 [`src/runtime.rs`](../src/runtime.rs) / [`src/interpreter.rs`](../src/interpreter.rs) detached spawn→yield→I/O wake 做可归因真实操作追踪，连同 permit、取消和 owner 的阶段保活计算全进程成本。BE 通知容器替换 **负结果**，新的单-job 服务入口 **未做**；没有可删除成本证据之前不增加拓扑或删公平 yield。

以上都是新数据/契约触发条件，不是下一轮自动实施列表。原数据、身份、负结果和适用条件以各链接报告及对应实验分支为准；本文件只读归纳，未改 landing 或生产源码。
