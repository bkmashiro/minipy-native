# CF：近期优化组合验收

结论：保留 BW、CC、CD、CE，结束这一阶段。四类合成请求在双后端的组合验收全部通过；没有发现具体正确性回归。native 算术循环的较低请求中位数提供局部支持，其他小幅差异和早期进程漂移不足以证明通用加速。CC 的保留依据仍是等待期间释放大 backing 的收益，不以延迟改善为前提。本卡没有生产代码修改。

## 固定比较与归因

A：pre-BW `c8d28f07ed9fbeb26444c308642767f34d7d56bc`。
B：main `1507ae8173842d980531f10abae1a1e92f398d49`。
A 是 B 的祖先。逐次检查区间内 `src/` 提交：BW borrowing、CC detachment、CD analysis、CE overflow exits 是保留的生产变化；BX 的 widening fusion、BY 的 Ready API、BZ 的孤立候选没有混入最终路径。BX 留下测试诊断，不代表保留其生产候选。CB 仍隔离在 `exp/cb-copy-patch` / `814fdd8`。

两臂使用完全相同的本卡 workbench adapter、16 项 Tool 注册环境、Provider、输入和默认 ScriptLimits。构建 A 时仅临时覆盖五个已有 `src/` 文件为 A 的精确 Git blob，随后恢复为 B 的精确 blob。CD 新增的 `src/interpreter/liveness_tests.rs` 文件在两臂 manifest 中相同，但 A 没有引用它，B 的引用是 `cfg(test)`；两臂正式 integration-test 库都不编译它。`tests/perf_workbench.rs` 是共同的 CF 测试覆盖，不属于历史 A adapter。没有新 worktree、target 或生产路径。

两臂采集时的 HEAD 标签都是 B；实际源码由每个文件 SHA-256 标识。reducer 分别与 A/B commit blobs 校验全部源文件，仅上述两个明确的测试覆盖例外，并校验共同 adapter 和 driver。不能将 A 的 HEAD 标签误读成 A 执行了 B 的源码。

## 四类请求与边界

- `branch`：新 scalar literal、条件分支及精确 stdout。作为短控制项，未覆盖 CD 的非零 owner mask 优化机会。
- `escaped_join`：Tool JSON escaped string，重复拼接同步 print；期望 `[hello\nerror/hello\nerror]\n`。
- `detach_wait`：每次请求内构造约 1 MiB JSON，捕获 11 B `hello error`，通过另一个 Tool 的真正 Notify gate 等待，然后校验参数并输出 `7\n`。
- `madd_loop`：每次生产/导入 256 Quotes，执行 computed-operand checked arithmetic loop，输出 `98688\n`。

`detach_wait` 的 gate、controller task 和专用 Host 在请求计时内创建和释放。Provider 标记 entered 后在 release Notify 上 await；controller 等 entered 并 yield 一次，确认状态仍是 blocked，再 release。Provider 恢复后读取参数、完成状态由请求断言；controller 也在计时内 join。没有 sleep 或硬编码等待时间，不声称真实远程 I/O 分布。此 adapter 扩展对两臂相同，但不与此前不同 Provider 表示的 BT/BZ 延迟直接混合。

每个请求从新 source 的 prepare 开始，包含执行、producer JSON/Quote 构造导入、Tool 等待调度、精确 status/stdout 断言、code/output 和临时 gate/Host 清理，终止后才生成报告行。调用方 source/expected 字符串构造、resident 环境/runtime/HTTP fixture 启动不在这个边界内。每次 source literal 不同，无 prepared-code/result 复用。正式二进制不带 compiler-profile/allocator/phase clocks。

A/B 与 exact-A A/A 各两轮 ABBA：64 进程、1,536 正式请求，另有每进程 sample 0 排除。所有进程 exit 0，所有状态、stdout、等待阶段断言通过；无缺失/失败。原始数据完整保留，没有增加样本追逐漂移。

## 描述性结果

B:A 是“各进程中位数的臂中位数”之比，不是因果估计或统计无回归界。

| backend / case | A µs | B µs | A/B B:A | exact-A A/A B:A |
|---|---:|---:|---:|---:|
| tape branch | 7.771 | 6.969 | 0.89676 | 1.01946 |
| tape escaped_join | 12.458 | 12.187 | 0.97825 | 0.99748 |
| tape detach_wait | 77.552 | 77.000 | 0.99288 | 0.99108 |
| tape madd_loop | 14.448 | 14.646 | 1.01372 | 0.99559 |
| native branch | 64.438 | 65.115 | 1.01051 | 0.99157 |
| native escaped_join | 79.157 | 78.041 | 0.98591 | 1.00673 |
| native detach_wait | 153.969 | 156.396 | 1.01577 | 1.00237 |
| native madd_loop | 125.646 | 115.875 | 0.92223 | 1.02027 |

A/B 最早的 tape branch 两进程中位数为 A 35.376 / B 31.084 µs，后续多数在 7–8 µs 左右，提示明显进程顺序漂移。branch 无 JSON/Buffer 非零 mask、没有 CE 多算术树，不能把 10.3% 描述性差异归为这几项的已证明收益。native loop 的四个 B 进程中位数均低于四个 A，方向与 CE 的局部 work/code removal 一致，仍不扩展为精确因果 7.8% 或真实 workload 的普遍收益。其余差异混合，等待项没有稳定延迟收益或确定性回归的证据。停止计时，不自动回滚 CC。

## 各项实际收益与限制

沿用已审阅证据，避免把不同指标相加或重新命名：

- BW：同步 sink 直接借用已准备文本，escaped join 的独立 execution allocation 16→15（双后端）。这里通过同类组合请求，无新的 allocation/density/RSS 计量。
- CC：先前 gated concurrent census 在每个后端 8 个唯一 Box-backed owner 上，等待前释放 8,388,968 B backing，留下 80 B copied 参数。含 half-guest cancellation 后继续读取、live/small/long/tight/escaped 对照。这里的参数是 11 B，历史 census 是 10 B，不能混为同一次计量。本卡重放旧 raw，另在当前 main 重跑原 fixture；没有新增 RSS 或真实等待 prevalence 结论。
- CD：已有 oracle 确认非零 DAG analysis visits 减半，如 buffer 20→10 / JSON 14→7，loop 36→36。CF branch 的 mask 全零，不用其延迟冒充 CD 专项因果收益。
- CE：先前真实 CLIF/VCode 每纯树 overflow exits 4→1，loop machine code 600→528 B，保留全部中间 overflow flag。本卡 native loop 有局部支持，不声明新 code-size 计量或全局吞吐改善。

BV schema/API 仍 defer，BX widening fusion、BY 每 Ready yield 的已测表示仍拒绝；compact fusion/amortized allowance 未测。CB 停止的仅是 tiny-glue/per-request module 设计，不是所有 C&P 路线。

## 验证与环境

macOS ARM64，测试/正式构建 Rust 1.96.0，`--offline --locked --release`，一个 build job。外置 `/Volumes/M.2` 已挂载，既有 `target` 指向 `/Volumes/M.2/caches/minipy-native-target`。构建/计时/完整 Rust 验证都经同一 fcntl build lock；`RUSTUP_AUTO_INSTALL=0` 在这些命令前设置。

`verification.json` 记录当前源码 hash、命令和退出码：CC fixture、完整 default 与 tool-json/debug-runtime release suites、严格 all-target Clippy、stable fmt、working-tree diff check 全通过。另九个 workbench Python contract tests 通过。原 error order、overflow、authority、预算、owner/cancellation 回归均保留。正式 gate 断言来自两个固定二进制本次实际运行。无 Linux/Wasm/Monty、RSS、真实 HTTP 性能或吞吐结论。

初次 adapter patch 因两个不唯一 hunk 被原子拒绝，没有改文件；后用独立上下文修正。新增 opt-in case 初次触发 case-list contract test，按新列表更新期望后九个测试通过。临时覆盖 codegen 的第一次 write 被 full-read 保护拒绝，改用 source patch 完成并逐 blob 验证；不涉及一次失败的正式构建/计时，也没有丢弃噪声观测。

工具链清理：枚举安装版本、no overrides、项目 TOML/toolchain 文件引用、进程和 `lsof` 检查后，对命名 `1.93.1` 的 49,691 个文件与 stable 逐文件 SHA-256 比较，仅 rustup inventories 两项不同；所有程序/库文件相同。未见活跃使用，遂只 uninstall 该误装冗余命名版。stable（仍 rustc 1.93.1）、1.92、1.96 和 active caches 不动，fmt/Clippy 在删除后通过。此前 `du` 为 1,282,516 KiB，未测量物理空间实际回收量。检查是当时、既述范围内的使用证据，不保证任意未扫描脚本不存在硬编码命名版。详见 `toolchain-cleanup.json`。

## 下一条最高价值问题

先确认真实 Host source 的长度、loop/wait mix 和 cold-prepare/reuse 比例，再决定是否值得改 preparation 表示。CF 的新源 native 请求约 65–156 µs，而 tape 约 7–77 µs；这只能显示总成本差距，不能单独归为 compilation。复用历史 BT 的独立 profiled Ready raw：48 请求 pooled median prepare 202.188 µs、backend 158.834 µs、execute+code release 11.813 µs；每请求 backend/prepare 比例中位数 78.7%。这是旧版本、带 instrumentation 的定位证据，非当前 CF phase share，且 medians 不可加。当前最需要的证据是真实 source 分布下准备成本是否仍主导、现有 tape 是否已满足需求；没有这个证据，不再排队做零碎 dispatch/CFG 微优化，也不据此承诺新 backend/cache 架构。

## 重放与复现

```
python3 tools/cf_stage_reduce.py
python3 tools/cc_verify.py final evidence/cf-stage/new-verification.json
python3 -m unittest discover -s tools -p test_perf_workbench.py
```

正式二进制复现：在干净 B checkout 保留同一 CF adapter；保存五个 `src/` 原件，将其覆盖为 A blob，构建 A，然后无条件恢复原件并验证 `git diff -- src` 为空，再构建 B。保持上述 inert test file，所有其他源码和 Cargo 文件用 reducer 验证来源。已有 output 路径不能覆盖，以下 scratch/raw 名称复现时须换新。

```
python3 tools/perf_workbench.py build --output /Users/yuzhe/.hermes/cache/scratch/cf-a
python3 tools/perf_workbench.py build --output /Users/yuzhe/.hermes/cache/scratch/cf-b
python3 tools/perf_workbench.py run --baseline /Users/yuzhe/.hermes/cache/scratch/cf-a --candidate /Users/yuzhe/.hermes/cache/scratch/cf-b --cases branch escaped_join detach_wait madd_loop --rounds 2 --reps 24 --output evidence/cf-stage/ab.jsonl.gz
python3 tools/perf_workbench.py run --baseline /Users/yuzhe/.hermes/cache/scratch/cf-a --aa --cases branch escaped_join detach_wait madd_loop --rounds 2 --reps 24 --output evidence/cf-stage/aa.jsonl.gz
```

`summary.json` 由 reducer 重建，含逐进程 medians/min/max、全源身份验证、旧 CC census 与旧 BT phase 归约。两臂来源标签有前述 explicit overlays；复现采集 HEAD 不必相同，但需相应调整固定 manifest HEAD 断言，不能静默跳过源码 hash。临时固定二进制在审阅后删除，原始 manifest 保留完整 binary/source 身份。
