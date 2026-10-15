# CG：真实 Code Mode 与下一片段决策

结论：停止新增语言/后端实现，保留 CF 的阶段结论。原 64 个脚本主要是通用 Python 对 shell/远端命令和文件工具的编排，并非当前有限声明 Tool + typed scan 的负载。它们确实来自 `execute_code.code`，不能因此重新归类成终端内 Python；但用它们证明有限 MiniPy 的兼容性或收益，必须先解决真实 Host 契约。没有证据支持继续数值循环微优化、准备缓存或新 native backend。本卡没有生产修改、构建、语料执行、真实 Tool 调用或新下游卡。

## 样本与方法

复用 `tools/bk_private_ast.py`，保持 Z 截止点、CLI 时间分位会话、call replay 去重和 BI/BK 最低 64 SHA256 规则。重新核对 CLI session hashes、replay counters、完整 `unique_exact` 聚合及 520 个源码的池大小，全部一致。不扩展到已漂移的其他 stratum，不重新抽样。BN 依据历史提交 `7616975` 的报告，BS 依据 `docs/experiments.md` 与 `tests/local_read_host.rs`，CF 依据其固定阶段报告。

仅对内存中的原 64 个完整字符串做 Python 3.14.7 AST 分析。两次独立提取/分析产生完全相同的固定标签聚合，计数守恒断言通过。私有探针只扩展原 BK 提取器，没有新数据管线；仓库仅保存 [aggregate.json](aggregate.json)，不保存源码、源码摘要、逐脚本结果、任务参数、会话标识或原始报错。源码身份与私有探针 SHA 在聚合文件中。

## 长度、结构与可观察重复

所有分布按不同完整脚本计，p90 为 nearest-rank，第 58/64 项。行数使用 `splitlines()`，包括空行/注释，不拆分嵌入字符串中的 shell/JS。

| 指标 | min | median | p90 | max |
|---|---:|---:|---:|---:|
| UTF-8 源码字节 | 230 | 626 | 1163 | 2019 |
| 源码行 | 1 | 4 | 11 | 22 |
| AST nodes | 17 | 98.5 | 262 | 421 |
| 全部调用语法位置 | 2 | 9 | 26 | 34 |
| 已识别 Host/shell 调用位置 | 1 | 2 | 4 | 8 |

后两项是 AST 调用位置数，不是动态调用次数、真实 Tool 次数或运行成本。Host 标签沿用 BK 的有限 helper 白名单，并识别显式 `from hermes_tools import ...` 别名；shell 标签仅识别 `terminal` 与 BN 所述外部 shell 适配器的调用名称。不做任意对象/别名解析，不把其他方法调用一律当成 Host。词法名称不能证明持久会话中的 callable 绑定或实际 effect。

- 61/64 有 shell helper 名称调用：28 有 terminal，55 有外部 shell 适配器，二者重叠 22。31/64 有其他已识别 Host helper。0/64 使用当前 `tools.<declared>` 调用拼写。
- 已识别 Host/shell 调用位置为 1 个的有 10 个脚本，2–4 个的 50 个，5 个以上的 4 个。所有调用位置合计 798：print 208、其他已识别 Host 50、shell 122、其他调用 418；不将此分布解释为时间份额。
- 11/64 有显式 for/while，共 16 个循环位置；20/64 有推导式/生成器，两类并集 25/64。3 个脚本在显式循环 AST 内含已识别 Host/shell 调用，推导式内为 0。AST 包含关系不证明循环会执行、每次执行多少次，或调用结果的数据类型。不能把所有循环都视为数值热循环。
- 0/64 有 await 或 async function。这反映同步 Python stub 的调用写法，不能推断无异步 Host 工作、无等待或 Ready 比例。
- 在原 CLI 池中，移除存储 replay 后的 520 个 `execute_code` 提交恰有 520 个不同源码；本 64 个源码各观察到一次提交。该窗口观察到的精确源码重提为 0，不等于生产执行器永不复用 prepared code，也不等于进程冷启动率 100%。未统计相似 AST/参数化模板，不推断未来缓存命中率或跨窗口复用。

这里没有 CPU、真实等待时长、返回数据大小、运行成功率、并发度、prepared object 身份或冷准备实际次数。CF 的总成本与历史 BT instrumented phase share 仍仅是各自合成证据，不能按本次源码字节长度外推。

## 现有功能之后还缺什么

没有重跑旧的无 schema 0/64 prepare。当前源码审计表明字符串 sink、JSON 静态叶子和四个有限 builtin 并未解决完整脚本边界：

- 64/64 至少有一个超出 `print/len/abs/min/max` 的 bare-name call，且没有当前声明 Tool 拼写。`src/frontend.rs:933–948` 只绑定 `tools.<name>`，`1236–1246` 的表达式调用仅接受有限 builtin。这是当前源级拒绝见证，不是新实测 admission 数字，也不靠伪造 Host schema 消除它。
- 49/64 有 keyword call，26 有 list/dict literal，20 有推导式/生成器，12 有 import，11 有 assert，2 有函数定义，3 有条件表达式。这些是重叠的进一步适配障碍；keyword 本身不是全局禁用，当前有限 Tool 可绑定自己的 keyword，不能把 presence 直接当成拒绝原因。
- 5 个脚本的 f-string 有 conversion 或 format spec，超出现有 sink 的有限格式契约。其余 f-string、subscript 或字符串 `+` 不再一律计为缺失：是否支持取决于已声明类型、静态路径和 consumer，AST presence 无法代替类型检查。
- BN 的 read helper 子集仍为 15/64，其中 13 同时有 shell helper。其余两个分别还含路径存在性/条件表达式，或 import/推导式/通用对象操作。一个 read Tool 加字符串 sink 不能使其中任何完整源码自动可运行。循环内 Tool 目前仍明确拒绝；不为 BN 在 64 之外的候选列表脚本暗中放开此策略。

因此，本卡确认启用的真实完整脚本：原样 0 个；显式改写后成功执行的 0 个（没有进行改写/执行，未来可适配数量未知）。前者依据完整源码的静态阻碍，后者是本卡交付计数，都不是新的 0/64 编译实验或总体覆盖率。单补 read helper 调用别名也不是这 64 个的完整解法。

## 唯一保留的设计场景与契约差距

若未来确有受限 Host 的需求，只保留一个已设计的完整场景：单个声明的只读文件读取 → 取静态 content 字符串叶子 → 有界 print。这就是 BS 已有 `tests/local_read_host.rs:127–237` 的完整 fixture，不是本次新增真实样本，也不再建议建设同一 fixture。

源码核对可确认该 fixture 的契约：Provider 只允许指定 ID；一个 path 字符串；固定可信 root 下的普通相对路径，canonicalize 后仍在 root 内；文件最多 4096 B、必须 UTF-8；返回 content 与 total_lines；越界/软链接逃逸 Permission，缺失/坏 UTF-8 Provider error，超限 Budget。现有测试对 native/tape 比较状态、调用顺序、stdout，并保留失败前输出。其历史执行证据见 BS；本卡只审读，没有重跑，也不把 canonicalize fixture 声称为 race-safe 文件系统沙箱。此场景已由当前语法支持，无需再加语言特征。

真正 Hermes `read_file(path, offset=1, limit=2000)` 不是该 fixture。重新审读本机 Host 的 `tools/code_execution_tool.py:129–131`、`tools/file_tools.py:502–532,605–755` 与 RPC 的 task-aware dispatch：

- RPC 返回字典；正常读提供分页/带行号的 content 和 total_lines。Host 保留任务路径解析、内部/特殊/二进制路径处理、文档提取、内容大小截断、脱敏和读追踪，不授予一个任意文件系统 capability。
- 返回不保证永远含 content：缺失/拒绝可为 error 或其他失败字典，重复读可为 `status=unchanged, content_returned=false` 的 stub，后续可升级为 refusal。不能把这些静默转成空字符串、重新读取以绕过 dedup，或任意改成 fixture 的 TaskError 后声称 source-compatible。
- 当前单字符串参数/静态 JsonTyped 路径并不覆盖整数 pagination 参数、可选/多形返回值与原 bare-name 绑定；也不能把 Python stub 的 import/持久全局视为自动提供。

Host 审计文件身份：本机 Hermes HEAD `135553abe96e74483860dad5c018ec75bb55ae3f`，以实际文件 SHA 为准：code_execution_tool `d576d4ef9b6cd2c18b37c3b1721d1927265575ec628f92e822729075aa3ce340`；file_tools `0aa29e6544ea7b30962db1f5482f85e7405fd417dc87210d5bb6838a0b421bbf`；code_execution_rpc `9b59c885947ff1a28e66bb0421c84c85768b8bc9f7ae71ce0cd7f940000a0a69`。这是源码核对，不是 Host 执行或完整安全审计。

## 下一步与停止门

现在停止实现。现有 Python/Monty 类执行器继续承担这些通用编排；有限 MiniPy 保留已有 tape/native 路径。若以后有一个实际 Host 所有者能提供完整、有限且授权明确的输入/返回/错误契约，再用一个完整源程序验证是否有价值；不要先为这批 shell glue 重建 import、远端执行和通用对象生态。对需要严格保留 Hermes 行为的场景，先由 Host 设计明确的返回 variant/error 策略；这是接口/语义决定，未经批准不实施。

只有实际完整场景成立后，才值得比较同一 Host/数据契约的 tape、native 与强简单替代，计入准备、执行、Host 转换和清理。当前未确定吞吐/内存目标，不能凭 626 B 的中位源码长度把 native 的编译成本判为值得，也不应增加缓存、框架或另一轮宽基准。

## 重放

在原本机私有输入仍可读时运行：

```
python3.14 -W ignore::SyntaxWarning ~/.hermes/private-research/minipy-trajectory-survey/cg_probe.py
```

该私有脚本通过 `runpy` 复用原 BK 提取器，只输出固定类别聚合；源码仅在内存中。仓库不复制私有脚本或原始输入。使用聚合文件中的 SHA 检测提取器/探针/前端漂移，保留原 CLI reconcile 断言，禁止跳过失败后继续出结论。报告只承诺此次两次重放一致，不保证可变本机历史库长期不变。无 Cargo，因此没有新增 target、worktree、下载、依赖、CI 或平台运行声明。
