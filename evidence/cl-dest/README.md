# CL：私有目的槽窄索引的单候选实验

保留这个最小表示改动，结束 CI–CL 的低层局部性微调阶段。确定收益是 tape backing 密度，未建立通用请求提速或无回归界。不继续压缩 Operand、搜索编码、引入缓存或重做已撤回的 CK。

## 范围与不变量

基线 `552faedd030d2a039b93aa04ca6005a3e991a0b3`，开始时 clean，与远端 `experiment/ci-layout` 相同。`main`/`origin/main` 仍为 `ebee3c1dfd87b4835f8bc16a00c2734f3dd53d69`，本轮不移动 main。

唯一生产候选是 `Dest::Local(u8)` / `Temp(u16)`。没有 `repr(packed)`、unsafe、常量池、变长编码、额外 metadata、执行器复制或新 dispatch。`Operand` 的 `Imm(i64)`、local/temp usize 和其他全部 Op 字段保持。两份生产文件合计 12 insertions / 9 deletions，精确差异见 `candidate.patch`。

Frontend 先以 `MAX_LOCALS=32` 拒绝额外 local/input，以 `MAX_NODES=512` 拒绝额外 AST node。每个 temporary 来自一个 admitted expression node，lowering 原有 `next <= MAX_NODES` 断言保留。唯一 local/temporary 构造点分别 `u8::try_from` / `u16::try_from`，在 preparation 检查，不用截断 cast；失败只表示内部 admission/lowering 不变量被破坏，现有普通用户输入仍得到 CompileError。消费时 `usize::from` 无损扩展，原 locals/temps 数组的 checked indexing 保留。Debug 的 destination 比较也 widening，保留 unchanged-value write trace。没有改变 lifetime、资源界、授权、取消、效果或错误顺序。

## 真实布局与实际代码

Apple M4 / macOS 26.4.1 arm64 / rustc 1.96.0。下列为本工具链的实际 Rust 布局，不是公开 ABI 保证。

| 对象 | A → B，bytes |
|---|---:|
| Dest | 16 → 4 |
| Op / alignment | 56 → 48 / 8 → 8 |
| Operand | 16 → 16 |
| Unit / State | 112 / 312，均不变 |
| scratch / native Frame | 4096 / 320，均不变 |
| 本具体 Provider tape / native future | 928 / 664，九类均不变 |

独立诊断临时复用 CI overlay/probe，九类双后端真实 prepare+execute，断言完整 stdout/status。Op 数、Vec capacity、buffer/JSON live length/capacity、native code bytes 均逐项不变。只减少每个 Op 的 8 B stride，backing 减少为实测 capacity × 8；未做 allocation census，不宣称少 allocation。

| case | ops / capacity | used tape A→B B | backing A→B B | prepared A→B B |
|---|---:|---:|---:|---:|
| scalar | 3 / 4 | 168→144 | 224→192 | 336→304 |
| bound_tree | 6 / 8 | 336→288 | 448→384 | 562→498 |
| madd_short | 5 / 8 | 280→240 | 448→384 | 560→496 |
| quote | 9 / 16 | 504→432 | 896→768 | 1088→960 |
| madd_loop | 11 / 16 | 616→528 | 896→768 | 1104→976 |
| ready | 7 / 8 | 392→336 | 448→384 | 1164→1100 |
| branch | 8 / 8 | 448→384 | 448→384 | 560→496 |
| joined_sink | 6 / 8 | 336→288 | 448→384 | 1350→1286 |
| async_string | 6 / 8 | 336→288 | 448→384 | 1059→995 |

Backing 相对少 14.29%，绝对只少 32–128 B。`prepared_bytes()` 是现有逻辑 owned-byte 估算，含其他 capacity/Plan 估算；不是 allocator resident bytes 或 RSS。

反汇编来自正式 A/B binary，各自 SHA 与正式 raw manifest 匹配，不用 layout binary 替代。`run_until_boundary` 的共享 dispatcher 仍各一个，A 1907 指令/7628 B → B 1875 指令/7500 B。两臂入口寄存器保存 96 B、4096+64 B 同步空间，共 4256 B，不因 Dest 缩小。没有 CK wrapper 或 scratch 大小分支。

实际 stride 从 A `mov w26,#0x38` 到 B `mov w26,#0x30`，仍用 `madd x19,x8,x26,x28` 计算地址。B 目的 tag 从 `[x19,#2]` 取，local 从 `ldrb w0,[x19,#3]`、temp 从 `ldrh w0,[x19,#4]` 无符号扩展到 W 寄存器，写 W 同时清零 X 高位；此实例无单独 `uxtb/uxth/uxtw`。例如 B offset `+0x147c` 的 `ldrh` 后仍有 `cmp x0,#0x200` 与 bounds exit，local offset `+0x1494` 的 `ldrb` 后仍有 `cmp x0,#0x1f` / `b.hi`。完整地址、字节和指令保存在 `diagnostic.json.gz`。静态指令少32不表示每请求少32次执行；不是 cache miss、动态 dispatch 或吞吐测量。

## 一次预设完整请求 campaign

沿用未修改 BT adapter/provider/schema/limits/driver，registered entry，七类双后端：scalar、branch、madd_short、madd_loop、ready、escaped_join、detach_wait。A/B 与 exact-A A/A 各一轮 ABBA，每进程24正式请求，sample0按原契约排除；共112进程、2688正式请求，全 exit0 且精确结果/等待 gate 断言通过。没有缺失、筛除或追加样本。

每次是新源码 prepare + 实际执行/等待 + producer构造导入 + code/output/临时Host清理。source/expected构造、resident环境/runtime启动在计时外，未缓存 prepared code 或结果。未插 allocator/phase clock/compiler-profile。

以下是进程中位数的臂中位数比值，仅描述性，不是精确因果估计：

| case | tape A µs | tape B µs | tape A/B B:A | exact-A tape A/A | native A/B | exact-A native A/A |
|---|---:|---:|---:|---:|---:|---:|
| scalar | 8.417 | 9.083 | 1.07919 | .98102 | 1.03288 | 1.02008 |
| branch | 13.000 | 12.333 | .94873 | .99862 | .97524 | 1.00765 |
| madd_short | 7.917 | 7.854 | .99207 | .99615 | .95338 | 1.00903 |
| madd_loop | 19.521 | 19.604 | 1.00426 | 1.01490 | 1.01936 | .99695 |
| ready | 14.365 | 14.250 | .99205 | 1.00730 | 1.01566 | .99684 |
| escaped_join | 13.511 | 13.386 | .99075 | 1.01703 | 1.01228 | .98955 |
| detach_wait | 78.011 | 78.052 | 1.00053 | .98739 | 1.00793 | 1.00675 |

必须报告 scalar +7.9%，不能用 A/A 抹掉；但首批两个 scalar 进程 A/B 中位分别13.813/15.104 µs，后两个为3.021/3.063 µs。branch 同样从18.521/17.167降到7.479/7.500 µs。单轮聚合明显混入 startup/时间漂移，精确A/A随后scalar为约2.7 µs。没有剔除首批或用事后子集重新估计候选。未改 native 路径仍有约−4.7%至+3.3%差异。总体不足以建立通用提速、确定因果回归或无回归界，不扩大campaign。

保留依据：极小私有表示变更直接少14.29% tape backing，Unit/future/同步栈不增加，实际单态 dispatcher 也没有膨胀；没有为这些字节加池、lookup、fallback或逐op新检查。延迟结果不能证明收益，也未在本预算下建立可信归因的回退。强简单替代仍是原56B Op，若以后有真实 workload 证据证明此表示有害，可直接撤回保存的patch，本轮不继续搜索。

## 正确性、身份与复现

新增 `tests/cl_dest.rs`：32 locals 的最后一槽写回、alias/短路/branch、600 rows/chunk7多turn、双后端独立期望输出；恰512-node admitted AST包含477 temporary slots，最高476，超过u8范围，真实 prepare/execute/debug单步均通过。多一层abs和第33local/input保持 CompileError，双后端错误文本一致。Debug单步还验证 `x=x` 虽值未变仍报告目的local写入。

单独 `bounds.patch` 临时接入 `bounds.rs`：通过原 Lower allocator 为所有512槽逐槽write/read round-trip，含511；32local同理。超出Lower上界仍断言，非法sealed local/temp仍触发checked-index panic。这只是内部表示/边界测试，不声称某公开源程序能使用512临时槽，也不把它计入性能workload。最终接入点已移除。

完整 default 和 tool-json/debug-runtime release suites、两套严格 all-target Clippy、stable fmt、9个Python workbench tests、diff check 全通过，实际 stdout/stderr/exit code在 `verification.json.gz`。原测试继续保护 owner/取消/权限/预算/overflow/JSON错误顺序。初始V4A edit因短上下文有两处匹配原子拒绝，无文件改变，读取后补足唯一上下文成功；没有正式构建/诊断/计时失败或丢弃观测。

所有Cargo/build/诊断/测量串行持有原 `~/.hermes/cache/scratch/minipy-perf-build.lock` fcntl锁；外盘已挂载，`target`保持原symlink，`CARGO_TARGET_DIR`为仓库target，`CARGO_BUILD_JOBS=1`，`RUSTUP_AUTO_INSTALL=0`，Cargo `+1.96.0 --offline --locked --release`。无新target/worktree/toolchain/依赖/下载/CI/webpage改动，无Linux/Wasm/Monty/真实Code Mode新结论。

只读复算（summary是派生文件）：

```
python3 tools/cl_dest_reduce.py
python3 -m unittest discover -s tools -p test_perf_workbench.py -v
```

Reducer从固定Git base+精确candidate patch重建正式、诊断与验证每个production source hash，核对adapter/driver、binary身份、进程/样本数、layout/future/code与backing invariant，并核对最终production源码与候选相同。

新采集从干净base先用原 `perf_workbench.py build --output NEW_A`，apply candidate.patch，再build NEW_B；按上列七类执行 `run --rounds 1 --reps 24` 的 A/B 和 `--aa`。`cl_dest_collect.py`在候选态临时reverse/apply保存patch复用CI诊断，finally恢复并hash确认；`cl_dest_verify.py`运行上限overlay和完整验证。脚本/raw均拒绝overwrite，复现须选择新binary/raw路径，不改写本轮身份。正式binary及sidecars在审阅后清理，共享target保持。

## CI–CL阶段结论

CI建立实际布局/代码基线；CJ保留zero-buffer prepare的一次allocation、一次pass和32–72 B backing删除，并报告边界分支代价；CK已撤回：小初始化写入交换更大同步栈和元数据/call，没有完整请求净值；CL仅保留当前简单目的槽表示和确定密度。各卡基线/fixture/时间不同，不能把它们的描述性latency delta相加，也没有声称CI→CL累计提速。

结束此阶段。当前tiny tape已远小于L1容量，CL仍可能改变局部访问，但没有PMU/RSS或实际业务收益证据支持进一步压缩。CG真实workload适配stop与BT/BX/BY/CB负结果不变，不自动重开已撤回候选，不增加缓存、后端或Tool适配。后续若有任务，应由新的实际Host source与端到端成本证据驱动，而非继续以布局字节为目标。

固定提交独立gpt-6-luna审阅无阻塞项，建议保留并停止阶段；实际model/provider receipt、只读重放及临时binary清理见 [review.md](review.md)。
