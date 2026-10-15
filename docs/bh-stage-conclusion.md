# BH 阶段结论（2026-10-15）

主结论：只集成独立的 BG 字符串成功验证缓存；record 旧路径隔离与 HTTP 通知微调未达到可归因的净收益门槛。一次性脚本的优化先删去已观测的重复处理，不能将 prepared 命中、数据类型或机器布局推成通用吞吐结论。

| 项目 | 决定 | 实际依据与限制 |
| --- | --- | --- |
| BG 借用 JSON 字符串验证复用 | 保留，已选择性合入 main | 原始 alias + 单次 contains 的三对 macOS 完整请求：16-KiB tape cold hit -15.48/-12.81/-12.93%，miss -24.32/-12.29/-9.53%；64-KiB miss -25.96/-25.41/-19.05%。native 的 16-KiB cold 和短串无稳定收益；短 warm native 三对约 +3.21～5.86%（40～80 ns），故不宣称全域提速。四对 BG 旧 scalar/Quote 控制无持续同向损失。首次成功解析建立 marker，owner 替换/释放清除；state 128 B、path 24 B 不变，但没有实测等待 RSS。保留原有预算、错误顺序和权限。 |
| BF C1 按需 record owner | 撤回，留实验分支 | 16 waiter 节省 632 allocator-requested bytes（39.5/task），4096 Quote warm 39,896→39,770 ns，未证明请求净收益；future 未减小。 |
| BF C2 core/record 双 AOT tape | 撤回，留实验分支 | 两轮 Quote cold/warm 均改善（65,417→60,667 / 40,270→35,416 ns；65,896→60,625 / 42,542→38,604 ns），这是有效本地观察；但额外执行体与二进制增量 16,480 B，热路径机制未隔离。静态整函数指令数不能否定动态提速，也不能独自证明值得维护。BD 遗留约 17% warm 差距仍未归因；AY/BD 不合 main。 |
| BE JoinHandle / event_interval | 撤回微调 | I/O driver、detached job 与 yield 存在交互；JoinHandle 没有跨单元稳定净收益，整进程 CPU 未下降。不能把约 13.43 µs detached+yields 全算为可删除的 channel 成本。 |
| HTTP 单 job 可剥离 continuation | 暂缓，不实现原型 | 尚无真实 operation task/channel 净成本占比，且新的 Host 服务入口必须证明 guest 立即取消、已提交 operation 独立完成并持续持有 permit、handle drop 不续跑 guest、ready 链公平。仅变更通知容器不是该原型。先取得可归因的服务负载证据，否则不增加新运行时拓扑。 |
| typed scan kernel / 局部 JIT | 暂缓 | 当前没有能通过代表性 typed 使用量及端到端回本门的输入；已有显式 tape/native 和 Host 有限 helper 是强简单对照，不新增选择层。 |

代码边界：main 从 `4e542619` 选择性集成 BG `f17cf8e`，最终签名 `8d14aca9`；生产 diff 只涉及 `src/json_view.rs` 和对应 `tests/string_contains.rs`，没有 AY/BD、BF、BE 的生产逻辑，也未改变 HTTP 或网站。完整方法、原始样本和局限见 `docs/experiments.md` 的 BG/BH 两段、`evidence/bh-string-integration/` 以及 BF/BE 的各自实验分支。Mac 相关全功能及无默认功能测试、严格 Clippy、fmt 通过；没有 Linux/WSL、真实远端 HTTP 性能或 gated RSS 新实验。此阶段停止，不凭上述局部数据创建 kernel/JIT/HTTP 开发任务。
