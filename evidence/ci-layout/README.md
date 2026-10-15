# CI：布局、实际指令与局部性诊断

## 决策

本卡只做诊断，生产源码、API、依赖和执行策略保持 `ebee3c1`。
推荐 CJ 仅实验一个更简单的候选：当 admission 已证明 `program.buffers == 0`
时，不构造、不扫描 buffer-only liveness 表。保留独立的 JSON liveness。
这是删除已知无用的 preparation 工作；尚未证明请求延迟改善。

第二候选是仅缩窄私有 `Dest`，暂缓实现。没有同时做 packed operand、常量池、
新 IR、布局框架、allocator、第三后端或 dispatch/fusion 改造。

CG 对真实 Code Mode 适配的停止结论不变。这组 fixture 是当前有限语言的
合成诊断，不是新增真实 workload 覆盖，也不构成继续扩大性能项目的授权。

## 身份与方法

- base：`ebee3c1dfd87b4835f8bc16a00c2734f3dd53d69`。
- Apple M4，macOS 26.4.1 arm64，`rustc +1.96.0` 实际为 1.96.0。
- `/Volumes/M.2` 已挂载；仓库 `target` 仍指向
  `/Volumes/M.2/caches/minipy-native-target`。未新建 target 或 worktree。
- 构建和全部诊断串行持有
  `~/.hermes/cache/scratch/minipy-perf-build.lock`；`CARGO_BUILD_JOBS=1`，
  `RUSTUP_AUTO_INSTALL=0`，Cargo `--offline --locked --release`。
- `probe.rs` 和 `loop-probe.rs` 只通过保存的 `diagnostic.patch` 临时接入
  lib tests。最终已移除接入点；没有将诊断代码编入正常生产路径。
- 九类 source 逐一准备 tape 和 Cranelift，运行两种实际执行器并断言完整
  status/stdout。JSON 来源为小固定 Provider，string Tool 使用真正一次
  `yield_now` 的 async future；不是 HTTP、真实业务服务或长等待密度实验。
- 长循环单独启用 `interpreter-profile`，计数与普通布局/反汇编构建分离。
  这些构建没有正式请求计时，也不混入历史 A/B 的样本。

`raw.json.gz` 保留命令、stdout/stderr、exit code、全部生产源 hash、主 probe
hash、实际二进制 hash。`summary.json` 由 reducer 重建。`tape-aarch64.txt.gz`
是 Xcode LLVM 对实际 release libtest 的反汇编，仅缩短反复出现的 Rust
函数名，保留地址、offset、指令字节。native raw 同时保留实际 backend
生成的寄存器分配后 VCode、CLIF 和 `code_info.total_size`。
VCode 有 island/relocation 等伪指令，因此不将其行数当作机器指令数。

早期诊断 raw 分别保存在 `diagnostic*.json.gz`：最初只构造 future，没有
poll，故未发射 tape 执行函数；随后补充两后端实际运行。另一次 objdump
同时使用 `--demangle` 与 mangled symbol 导致空输出，改为 raw symbol 后
成功。最终 `raw.json.gz` 完整包含所有九类执行、非空反汇编和六个循环
计数断言。没有正式计时样本被筛除。

## 实测布局与边界

| 对象 | 实测大小 B | 含义 |
| --- | ---: | --- |
| `Op` / alignment | 56 / 8 | 固定 stride，所有 variant 付最大布局成本 |
| `Operand` / `Dest` | 16 / 16 | 带 tag 的叶子/目的位置；Operand 需完整 i64 literal |
| `InterpreterUnit` / `State` | 112 / 312 | 对象本体，不含所指向的 heap backing |
| tape scratch | 4096 | `[i64; MAX_NODES]`，同步执行 turn 的栈上临时对象 |
| native Frame / View | 320 / 16 | Frame 被 Box 持有，View 另有动态 Vec |
| Output / JSON State | 72 / 128 | 不包含字符串、owner backing 等动态内容 |
| tape/native execute future | 928 / 664 | 本卡具体 Provider 单态类型，九类 source 相同 |
| 仅窄化 Dest 的镜像 Op | 48 | 全 variant 集合、其他字段不变的独立布局模型 |

future 的 928/664 B 是 `size_of_val`，不是每任务 heap 总量或 allocator
resident size。scratch 不跨 await，不应加到等待 future backing 上。
本卡没有 allocation-count profiler、blocked-task census 或 RSS 测量；
不将既有 CC 的 80 B owned string argument 与此处完整 future 混为一谈。

| source | ops | used / capacity tape B | buffer live B | JSON live B | prepared B | native code B |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| scalar | 3 | 168 / 224 | 32 | 0 | 368 | 164 |
| bound_tree | 6 | 336 / 448 | 56 | 0 | 618 | 212 |
| madd_short | 5 | 280 / 448 | 48 | 0 | 608 | 188 |
| quote | 9 | 504 / 896 | 80 | 0 | 1088 | 464 |
| madd_loop | 11 | 616 / 896 | 96 | 0 | 1104 | 540 |
| ready | 7 | 392 / 448 | 64 | 32 | 1228 | 400 |
| branch | 8 | 448 / 448 | 72 | 0 | 632 | 236 |
| joined_sink | 6 | 336 / 448 | 56 | 28 | 1406 | 352 |
| async_string | 6 | 336 / 448 | 56 | 28 | 1115 | 444 |

表中 live B 按实测 capacity × 元素大小计；prepared 来自现有
`prepared_bytes()`，含 unit/Vec capacity/绑定 Plan 估算，不是进程 RSS。
前述两个循环有一个 Quote buffer，其他七类均为 zero-buffer。Quote row
owner、JSON root owner、compiled code allocator 及共享 helper 的成本
不能用本表的 code/metadata 大小代替。

## 实际执行成本与局部性

实际 tape 函数含 1907 条 AArch64 指令，即 7628 B，包括共享分派、所有
variant 和错误/越界冷路径；它不是每条脚本执行的指令数。
反汇编的入口和 dispatch 序列（地址以本次二进制为准）：

- `0x10001742c`：为同步 scratch 预留约 4 KiB 栈空间。
  `0x10001744c..454` 对 4096 B scratch 发出初始化调用。
- `0x100017480`：stride 56；`0x1000174b4` 计算 `ops + pc * 56`。
- `0x1000174b8..4d0`：PC 加一及写回 State，load opcode tag，load jump
  table entry，计算目标并间接 `br`。每个 op 的两个 operand 解码继续加载
  tag/payload，并保留 local/temp/row 的 bounds/lifetime checks。
- 比如乘法路径 `0x10001791c..928` 有 `smulh`、高位/符号扩展比较及
  overflow branch。没有把机器上的 wrapping `madd` 当成 checked 语义。

这些是实际静态指令证据，尚不能归因为 interpreter 全时占比、branch-miss
或 D-cache miss。没有逐行复制 56 B enum 的证据；不能根据类型 size 把
“整条 Op copy”列成已发生的内存搬运。

Quote loop 的 native VCode 显示 row 字段两个 load，accumulator 在寄存器，
checked mul/add flags 通过 OR 后共用一个 overflow exit，循环内没有
每个表达式都回写 Frame。Frame spill/resume 和预算更新位于 chunk 边界。
这与 CE 方向一致，不重复提出四个 error exits 合并候选。

本卡实跑 256 / 4096 / 65536 rows，两后端均核对输出：

| rows | Quote payload B | quote dispatch | computed dispatch | reservations / yields |
| ---: | ---: | ---: | ---: | ---: |
| 256 | 4096 | 1030 | 1542 | 1 / 0 |
| 4096 | 65536 | 16390 | 24582 | 16 / 15 |
| 65536 | 1048576 | 262150 | 393222 | 256 / 255 |

计算路径分别为 `4*n+6` / `6*n+6` dispatch，chunk 固定 256。
这是 instrumented 动态计数，不是性能加速比。行数增加时 tape 本身仍为
504/616 B used，增长的是连续 Quote row streaming 与 dispatch 次数。
主机 `sysctl` 实测 cache line 128 B，L1I 128 KiB、L1D 64 KiB、L2 4 MiB。
这些 tiny tape 在容量上远小于 L1D，但没有实测地址冲突、cache hit 或
miss rate，不能因较小就声称“已经没有 cache 问题”。

未采集 hardware cache/branch misses。`xctrace list templates` 能枚举
CPU Counters，但本轮未配置/验证相应 PMU event 或采集权限，不将“列出模板”
等同于事件可测，也不假称 macOS 没有计数能力。perf/Linux、Wasm、Monty、
真实 HTTP 和 production workload 的性能均未新跑。

hot/cold 判断以具体访问为界：op/operand 和 State 在同步循环里反复触碰；
buffer/JSON live masks 在 effect/yield 边界由 dispatcher 使用。
16 项注册环境不表示每任务 retain 全部 16 项声明，prepared Plan 只含绑定
结果。为小 tape 再加 metadata pool/lookup，或者每请求 shrink Vec，都可能
增加本已很短的 preparation、allocation 和间接访问，当前无证据支持。

## 候选排序与 CJ 契约

### 1. 选中：跳过已证明空的 buffer-liveness 工作

依据：`src/interpreter.rs::from_program` 在 `buffers==0` 时仍分配
`ops.len()+1` 个 u64 并逆序扫描 `Op`；七个 zero-buffer fixture 实测仍有
32–72 B backing。对应 mask 恒零由 admission namespace 与 transfer
规则证明，不是按 workload 猜测。JSON liveness 已独立条件化，不能一起删。

预期可删除：每个 zero-buffer preparation 一次 Vec allocation/zeroing、
一次 reverse traversal、32–72 B live backing。数值来自现有容量；这是
源码推导的可删工作和上限，不是候选已实测的 allocation delta 或 ns 收益。
保持 Vec header 不变时，Unit/State/future 本体不预期缩小。

强简单替代：当前 tape 不变。与窄化指令相比，此候选去掉明确 allocation
与整次 traversal，不依赖 cache miss 假说，也没有 decode/pool 成本。
只值得一轮 bounded 验证；若完整请求无法看出净值，保留当前实现并停止。

CJ 的最小实现边界：

- 只改变 buffer-count 为零的私有 metadata 构造/访问，非零 buffer 分析和
  释放规则完全保留。无 public API、bounds、capability、fuel 或 effect 改动。
- 统一处理所有正常 dispatcher 和 `interpreter/debug.rs` 的 mask 读取。
  zero-buffer 返回概念 mask 0；非零 buffer 继续原有有效 PC 的索引约束。
  不使用通用 `get(...).unwrap_or(0)` 静默掩盖非零 buffer/PC 错误。
- CD oracle 应继续逐 PC 验证逻辑 mask，而非强行要求空存储等于旧 Vec。
  增加 zero-buffer scalar/branch、Ready JSON、async string Tool，以及有
  Quote 的 branch/loop 对照；保留 cancellation、permission、error、
  output-cap、JSON-root 与 escaped argument 的原测试。
- 分开采集 allocation/metadata/count 诊断和正式请求时间；执行既有
  workbench 的 exact-base A/B + exact-A A/A，预先固定两轮 ABBA、每 cell
  24 正式 requests（另有原 harness 的 warmup），不追加到显著为止。
  建议 cells：scalar、branch、ready、escaped_join、detach_wait、quote、
  madd_loop，双后端、同 adapter/provider/schema/limits；native 和有
  buffer 的 loop 是重要未改变路径对照。
- 计时包含新源 prepare、完整执行/等待与资源释放。只在 zero-buffer tape
  preparation 预期少一次 allocation，native 和非零 buffer 应保持；
  若需要额外池、unsafe、通用 fallback 或新通路才能成立则停止。
- 不凭一次小于 A/A 漂移的延迟差宣布胜利。按原卡的正确性、端到端和密度
  标准给 keep/revert/defer，不能以 32–72 B 自动折算 RSS 或吞吐。

### 2. 次选且暂缓：只窄化 Dest，完整 Op 56 → 48 B

完整镜像枚举只将 `Dest::Local(usize)/Temp(usize)` 换为 `u8/u16`，实测
Dest 16 → 4 B，Op 56 → 48 B，理论上每 capacity op 少 8 B，本表为
32–128 B。`MAX_LOCALS=32`、`MAX_NODES=512` 是 admission 的既有上限，
无需压缩 i64 literal 或新增常量池。若未来实验，构造处必须证明范围或
checked conversion，不允许截断，也不改热路径 spatial checks。

只证明了模型布局，尚未实现/执行候选，也没证明 LLVM 的 load/branch
数量、I-cache 或时间改善。窄化目的地不消除 Operand 的 tag 和 16 B
literal 容器，不减少 dispatch 次数；对 tiny tape 可能只有小幅 backing
收益。CJ 本轮不捆绑它，更不做 packed/variable-width bytecode。

## 保留的负结果与边界

- [BT workbench](../../docs/perf-workbench.md)：历史 instrumented Ready 的
  backend/prepare 中位比例 78.7%，不是 CI 当前 phase share。
- [BX](../bx-madd/README.md)：generic checked multiply-add fusion 未保留。
  本轮没有扩大所有指令布局以减少一个 dispatch，也没有重做其正式计时。
- [BY](../by-ready/README.md)：Ready 兼容桥的组合延迟和 future/branch 成本
  不支持保留。此处 async Provider 没有新 Ready trait/associated-type 桥。
- CB 保持隔离在 `exp/cb-copy-patch` / `814fdd8`。已读取该分支实验记录：
  raw stitch/install floor 中位 3.98925 us，tiny full tape 3.542 us；raw arm
  不含完整 source/Tool/output。未重复 installer gate，更没据此批准第三后端。
- [CF](../cf-stage/README.md)：组合验收保留 BW/CC/CD/CE，不能把历史混合
  漂移当作此次候选因果收益。CC backing/RSS 边界和 CG workload-fit stop
  结论继续适用。

## 复现与验证

纯归约，不构建、不执行计时：

    python3 tools/ci_layout_reduce.py
    git diff --exit-code ebee3c1 -- src Cargo.toml Cargo.lock
    git apply --check evidence/ci-layout/diagnostic.patch

完整重新诊断（会使用相同 external target；先确保没有其他运行）：

    git apply evidence/ci-layout/diagnostic.patch
    python3 tools/ci_layout_probe.py --output /绝对路径/新的-ci-raw.json
    git apply --reverse evidence/ci-layout/diagnostic.patch

runner 对已存在的 raw 输出拒绝覆盖；重新运行会刷新本证据目录内未压缩的
`tape-aarch64.txt`，不要把新 raw 与旧汇总混用。上述 apply/reverse 不构建；
runner 内部所有 build/run 都持有 perf lock。无论成功与否都应移除 overlay。
如需重放 reducer，先归档新 raw/反汇编再比较，不能冒充本次固定身份。

本次最终证据：九类 × 两后端 status/stdout 验证通过；六组循环 × 两后端
验证通过；tool-json lib suite 28 passed / 5 ignored；单独两个 ignored
诊断与一个 instrumented 诊断均通过。reducer 验证每个源码 hash 可由精确
base blob + 保存的 test-only patch 重建，且当前所有 `src/*.rs` 与 base
逐字节相同。最终检查记录另见 `verification.json`。
归约、patch apply-check、Python compile 和既有 workbench 的九个单元测试
通过。1.96.0 未安装 cargo-fmt，首次格式检查因缺组件退出；未下载组件，
改用已安装的 stable rustfmt 后 production 与两个 probe 的格式检查通过。

完成诊断后在原 CI 卡请求 review，CJ 等验收后继续；不新建重复 review/
优化卡，不把本卡标成生产优化完成。
