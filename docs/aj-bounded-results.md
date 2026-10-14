# AJ：有界热点试验与 Tool 结果直接输出

## 结论

保留一个 opt-in 小能力：`print(tools.publish(total=n, batch_id=batch_id))`。它在准备期展开为现有的 Tool 调用、等待、标量 print；不增加执行指令、helper、动态 Value 或后端。声明的返回值必须为 i64。AH 已有 keyword 绑定和常量字符串键访问，本轮没有重复实现，也没有扩成数组语言。

唯一性能候选是避免调用绑定时克隆整份 ToolSchema。四组 AB/BA/BA/AB 未显示 source-cold 净收益，已撤回，不继续第二项。候选 patch 与所有观测均保留。旧 scalar/weighted cold 的 AH 大幅回退在这轮小控制中没有稳定重现，不能据此宣布已修好。启用实际 I/O reactor 后扫描 yield 的额外成本仍然清晰可见，没有改 yield、默认 chunk、预算或取消语义来掩盖它。

本分支基于批准的 opt-in AH `f7167add303a52de73f449d1ef5c57df9552e2f4`。未合并 main，未改网站、冻结 bench、AF/WSL 工件或 Pysolate。解释/native 仍由 Host 显式选择。

## 价值与范围

本机私有 trajectory-survey 的简报显示 Tool 调用、输出和索引是值得补齐的结构；AH 已覆盖其中两项。本轮只连接已有的 scalar Tool 返回值与已有输出入口，允许普通 Python 的直接调用打印形状，减少一次显式临时绑定。没有发布或复制原始轨迹，也没有声称它使任何完整真实脚本兼容。能力证据是两种写法在 native/tape 的差分测试和真实 localhost fetch→计算→publish→print；未跑完整真实轨迹成功率评估。

AH 的最多两个 i64 Tool 参数仍是当前有界 Host ABI 的实现上限，不是 Python 语言规则。本轮没有增加参数限制、专有调用写法、JSON print 格式或新容器。任意表达式中的 Tool、嵌套 Tool、Tool-in-loop、JSON 根直接 print 仍拒绝。`print` 本身仍只接受一个参数；Tool 参数先按源码顺序求值，再按声明绑定，Tool 完成后才执行 print 权限/输出预算检查。已有 stdout 在后续错误或取消时保留。

## 热点、简单替代与停止条件

观测依据：AH 小 JSON prepared 请求约一微秒，native cold 则约 85–470 微秒；字段缓存已存在，全数组 typed decoder 已明显胜过 lazy。源码中 `compile_tool` 为每次调用 `.cloned()` 整个声明，再保留所需 JSON leaf schema。克隆了用完即丢的名称与参数字符串，这是具体可移除的准备期开销；它不在执行热路径，不应带来 prepared 提速。

最简单替代是保持 AH。候选仅用声明索引读取 id/参数元数据，只复制必须保留的 JSON schema。预期仅省几次小分配，收益上界很小，不能把全部 cold 时间视作可省。停止条件为小配对中 source-cold 无稳定净收益，或引入额外状态/分支；未设事后显著性门槛，也不为负结果增加特例。

四组独立进程 AB/BA/BA/AB，候选/基线两侧相同 feature、工具链与 time-only 配置：

| JSON workload | backend | baseline cold µs | candidate cold µs | 变化 |
|---|---|---:|---:|---:|
| few fields | native | 88.789 | 90.489 | +1.92% |
| few fields | tape | 10.485 | 10.640 | +1.48% |
| repeated | native | 488.101 | 500.110 | +2.46% |
| repeated | tape | 76.957 | 78.987 | +2.64% |

Repeated cold 两个 backend 四对都变慢。无调用绑定的 prepared/旧扫描控制也有变化，说明布局/进程噪声不可忽略；没有将其归因为已证明的某个硬件或编译器原因。候选省分配的源码推断不足以抵偿端到端负结果，故撤回。最终生产 diff 只有 frontend 的直接 print 展开，runtime/codegen/tape/JSON/HTTP/provider 文件均与 AH 相同。

## 旧路径资格控制

同一 `fastpath_probe.rs` 在 pre-AH `c3dff6a` 默认、最终默认、最终 tool-json 三种构建执行；四轮前后反序，全部 checksum 相同。两种默认构建都不启用 I/O，feature 构建在此表也只启用 time；实际 I/O 控制另表保留。

| workload/backend | lifecycle | pre-AH default µs | final default µs | final tool-json µs |
|---|---|---:|---:|---:|
| scalar/native | cold | 71.147 | 65.544 | 66.586 |
| scalar/tape | cold | 9.366 | 9.105 | 9.148 |
| weighted/native | cold | 153.193 | 151.452 | 150.822 |
| weighted/tape | cold | 26.923 | 26.107 | 24.511 |
| stateful/native | cold | 166.568 | 166.264 | 164.146 |
| stateful/tape | cold | 30.461 | 31.030 | 28.421 |
| scalar/native | prepared | 0.186 | 0.169 | 0.171 |
| scalar/tape | prepared | 0.195 | 0.179 | 0.186 |
| weighted/native | prepared | 1.959 | 2.067 | 1.906 |
| weighted/tape | prepared | 8.512 | 8.472 | 7.872 |
| stateful/native | prepared | 2.035 | 2.120 | 2.065 |
| stateful/tape | prepared | 7.274 | 7.531 | 7.305 |

这轮没有出现 AH 报告的 scalar cold +20–33% 或 weighted tape cold +20%。但只是四轮小批次，最终默认 prepared weighted native 相对 pre-AH 仍高约 5.5%；没有全路径零成本承诺。原 AH 结果不覆盖、不作废。native code size 三组均为 scalar/weighted/stateful 264/552/576 B，不能把尺寸相等说成机器码逐字节相同。默认 tape retained logical bytes 与 pre-AH 相同；启用 tool-json 仍多 32 B/unit，属于 AH 已有 plan/liveness header，不是本轮新增。旧程序不创建 JSON State/owner/cache 或 HTTP client。

同一最终 feature-enabled 可执行文件，ready Quote provider、1000 行和默认 chunk=256，两侧保留同样的三次 fairness yield：

| prepared | time-only µs | I/O-enabled µs |
|---|---:|---:|
| scalar native | 0.175 | 0.175 |
| scalar tape | 0.182 | 0.192 |
| weighted native | 1.951 | 16.157 |
| weighted tape | 7.842 | 21.603 |
| stateful native | 2.102 | 16.208 |
| stateful tape | 6.973 | 21.430 |

I/O-enabled 扫描多约 14 µs/request；这与 AH 的 reactor 控制一致，但不是固定普适常数。非 I/O 应用可选择只启用 time；真正 HTTP 必须启用 I/O，本轮没有提供规避其成本的新调度器。compile/runtime optional 状态分支仍存在，无法从这些批次分离其纳秒级代价与代码布局、进程噪声，不捏造归因。

## 新能力：实际 HTTP 与 retained 成本

`examples/aj_http_probe.rs` 使用真实 TCP、共享 HttpService/连接池、静态两个 endpoint、声明 Tool 和 JSON i64 叶。响应 fixture 固定为有效小 JSON；服务端与客户端在同一 current-thread Tokio runtime，服务端工作也会计入墙钟。每次源码请求执行 fetch、字段乘法、publish 和 stdout 校验/消费/清理，两个真实 HTTP 请求。Host/fixture/池构建在计时外，每批前一次 warm-up 建立池；prepared 不含准备，cold 每次重新准备。无真实远程 API/互联网/TLS 性能结论。

三独立进程，每单元三批；每批 cold20/prepared100。下表为三个进程均值的中位数，非 p95/p99。两种语法按固定先 assigned 后 direct 顺序，因此不是可归因的语法提速试验。

| syntax/backend | cold µs | prepared µs | tape logical bytes / native code bytes |
|---|---:|---:|---:|
| assigned/tape | 112.138 | 92.075 | 990 |
| direct/tape | 94.876 | 82.578 | 990 |
| assigned/native | 196.324 | 89.957 | 492 |
| direct/native | 202.606 | 90.436 | 492 |

全部 72 batch rows 的输出 `27\n`、请求参数、response 计数、归还 permit、每进程一条连接检查通过。HTTP fixture 数值只是语义资格与真实等待成本证据，不能用固定响应推导业务 API 吞吐。记录实际 merge-copy 计数；这不是 TLS/kernel/完整传输的零复制承诺。

另跑最终独立 allocation diagnostic（不使用其时间）：prepared JSON few tape/native 为 13/13 allocation calls、1164/1420 cumulative requested B；repeated 为12/13 calls、1100/1420 B。每单元只有一次诊断，不是分布。两个语法的 tape bytes/native bytes 相同，没有新增等待帧或运行时对象。逻辑 unit bytes 与 native executable bytes 边界不同，不能直接等同常驻内存；本轮未测 RSS/PSS/最大任务数。既有 root identity、跨等待保活、last-use、detach late completion 测试继续通过。

同轮强简单对照仍为 typed decoder：1000-row array serde typed 33.662 µs、sonic typed34.102 µs，lazy164.366 µs、row-slots129.941 µs。少字段 lazy0.153 µs、sonic typed0.485 µs、serde typed0.539 µs；输入已驻留且 validity/duplicate 语义并不完全等价。native JSON 仍跨共享 parser/cache helper，新的直接 print 不改变它；不声称通用 native 优势，不增加对象游标/融合器来救负结果。

## 证据、验证与复跑

机器为本机 Apple M4/macOS26.4.1，rustc/cargo1.93.1；依赖锁未变。只用已有 target，CARGO_BUILD_JOBS=1，全程 offline 构建，无下载/WSL/VM/付费资源。可用磁盘约12→11 GiB；没有复制 target。临时可执行文件约数百 MiB 留在本卡工作区，不进 Git；raw/摘要只保存一份 canonical CSV，无第二份 raw JSON。

`evidence/aj-bounded/`：候选984行、最终984行、默认控制144行、HTTP72行、独立allocation33行，共2217个 batch/diagnostic rows。每行包含多次操作，不能称为2217个逐请求 latency 样本。四组顺序与所有配对方向保留，未筛除较慢进程。summary 可用 `python3 tools/aj_verify.py` 从 CSV 独立重算，exact-match 检查不重跑实验。`candidate` 目录是已撤回 schema 克隆优化；`final` 中 snapshot 字段 candidate 代表最终语法变更构建，具体 binary hash 以各目录 identity 为准。

验证：最终 macOS all-feature89项/default63项、strict all-target/all-feature Clippy、fmt、Wasm debug lib check 通过。新增三个测试覆盖 direct/assigned差分、参数绑定、stdout预算与前缀、Tool权限/调用预算、Provider错误、参数错误先后、真正挂起与取消、JSON/nested/loop拒绝。Linux未运行；既有AH Linux边界不提升。没有重跑冻结 benchmark 矩阵。

复跑构建均先 `export CARGO_TARGET_DIR=/Users/yuzhe/projects/minipy-native/target CARGO_BUILD_JOBS=1`，按顺序独占该 target：

    cargo test --offline --locked --all-features
    cargo test --offline --locked
    cargo clippy --offline --locked --all-features --all-targets -- -D warnings
    cargo fmt --check
    cargo check --offline --locked --lib --target wasm32-unknown-unknown --features debug-runtime
    cargo build --offline --locked --release --features tool-json --example json_probe --example fastpath_probe --example aj_http_probe
    cargo build --offline --locked --release --features json-allocation-profile --example json_probe
    python3 tools/aj_verify.py

A/B：在 AH 固定版本保存 json_probe/fastpath_probe 为 `../aj-base-{json,fastpath}`，应用 `schema-clone-candidate.patch` 后保存 `../aj-candidate-*`，运行 `python3 tools/aj_pairs.py .. evidence/aj-bounded/candidate`。撤回 patch 后使用本分支构建保存 `../aj-final-*`，用 `--new-prefix final` 输出 final 目录。默认控制另在本卡 detached pre-AH worktree 复制完全相同的 fastpath_probe，仅构建它、不动冻结 bench；最终 default 与 feature binary 按 `tools/aj_default_control.py` 指定名字保存，运行该脚本。HTTP运行三次 `../aj-final-http` 各保存 process-N.csv；allocation build单独保存csv。不要拿 allocation executable 代替 timing executable。

停在此交付点：没有净性能优化保留，没有第二轮候选或后续任务。签名推送独立分支后交定向 review，由主控决定是否合并。
