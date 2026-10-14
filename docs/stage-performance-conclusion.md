# 阶段性能结论：显式 IR 与 native

结论：保留共享前端/typed IR 和两种显式执行入口。一次性小程序优先考虑解释器；已明确复用或长扫描优先考虑 native。目前没有证据支持自动 JIT、缓存命中假设或新增轻 backend。本阶段到此停止，不新增任务或第三个优化候选。

## 能力与比较边界

两 engine 支持有界 Python-like 标量/顺序扫描、checked i64、短路、私有 stdout、真实异步 Tool 和 chunk 让出。共享数据所有权、权限、取消、累计预算与错误契约；不支持完整 Python、一般对象/函数、嵌套扫描或循环内 Tool。可信编译器、runtime、Provider 仍属 TCB。

固定 Rust 参考只执行 scalar/weighted/stateful 三个预编译程序，共享 Provider/oneshot、typed buffer、输出、权限、预算及取消路径。它不编译未知源码，不能替代通用 baseline，也不是物理下界。其 coroutine/frame 表示不同，trace frame_bytes 仅为估计；等待分配及 RSS/PSS 才是密度证据。普通 Monty 仍是未知源码外部对照；i64 与任意精度整数等语义边界需保留。

## 同轮实数

现有独占 ARM64 Linux VM（2 vCPU、2048 MiB），沿用 H 的 synthetic light/scan 权重、有限到达窗口，三轮交替；不是生产画像或持续服务容量。145,200 条 raw：baseline 72,600，两候选各 36,300。199 个 baseline 超载窗口保留。下面 CPU 是全进程每完成任务 CPU，单位 µs；warm 表示调用者显式复用，非自动缓存。

| shape | 参考 cold / warm CPU | native cold / warm CPU | IR cold / warm CPU | Monty cold / warm CPU |
|---|---:|---:|---:|---:|
| scalar | 4.45 / 9.18 | 233.55 / 3.35 | 52.29 / 3.45 | 54.17 / 42.58 |
| weighted 100 | 14.86 / 16.10 | 349.97 / 13.40 | 98.10 / 22.65 | 216.68 / 197.28 |
| weighted 10k | 113.84 / 82.53 | 408.20 / 85.46 | 611.33 / 335.46 | 3878.72 / 3777.28 |
| stateful 100k | 509.05 / 504.09 | 839.77 / 581.67 | 2500.07 / 2542.49 | 37658.68 / 35301.68 |

参考 cold 不含源码处理。小 shape 存在显著频率/调度噪声，不能从 native 偶尔快于参考推导通用最优性。100k prepared native 比参考多 77.58 µs CPU，IR 多 2038.40 µs。native 的长扫描接近共享 Host 路径成本，IR 表达式分派仍有较大空间。

准备与执行 wall 子区间不等于 CPU 分解：scalar cold native 准备/执行 225.37/6.26 µs，IR 41.62/6.92；100k cold native 281.00/558.69，IR 38.16/2464.03。未知小源码的主要 native 成本在准备。

群体 cold 编译在单线程上与执行共队列。light ready cold 的 IR/native CPU 为 588.31/688.49 µs，平均到达至完成延迟 2116.50/10419.39 µs，短任务 p99 为 2701.58/23484.29 µs；warm CPU 为 518.35/176.14 µs。scan cold CPU 为 837.01/848.21 µs，平均延迟 4168.26/18883.53 µs；warm CPU 为 838.71/286.08 µs。这些差异包含到达、timer、队列与生命周期；异步 sleep 不计为 CPU 收益。完整 p95/p99、delay=1000 和逐轮结果见详细报告。

record 等待时已释放 Quote payload。light/scan 相对启动的 live requested 分配：参考 53,686/69,608 B，IR 86,134/124,264 B，native 181,436/301,874 B。record RSS：参考 2,297,856/2,330,624 B，IR 3,534,848/3,584,000 B，native 5,451,776/5,697,536 B，普通 Monty 39,235,584/71,811,072 B。采样峰值 RSS：IR 7,090,176/10,690,560 B，native 9,003,008/12,800,000 B，Monty 77,357,056/132,685,824 B。JIT mappings 不在 requested allocation 内，不能据此推算最大并发数。

## 两个候选与止损

1. 叶操作数内联：100k weighted/stateful 减少 520,172/427,064 次递归 eval 入口，六个混合 CPU 单元及每轮方向均改善，汇总约 4–14%。但 ready 短任务 p99 在 light/scan 各三轮均上升，汇总 2561.46→2990.80、3467.98→4095.89 µs；其他尾部及小 shape 混杂。保守撤回并保留 CPU 正结果。三轮波动不能证明稳定因果退化；验收不是要求每个 cell 都改善。
2. 单恢复入口 native 裁剪：消除 PC load/default dispatch，辅助 macOS scalar code 264→236 B，混合 CPU 没有一致净收益，撤回。无需为极小代码缩减重做 backend。

生产默认无计数/allocator profile 开销。Linux 事件计数和辅助 macOS 采样共同定位递归 eval，但没有 Linux 低扰动火焰图，不能拆出 bounds/local/fuel 的精确 CPU 份额。旧 single_pass 与 empty-else Jump 负结果保留，不重复尝试。

## 使用建议与后续优先级

默认工程建议：未知、一次性、小程序显式选择 IR；已知重复执行或较长 typed 扫描显式选择 native。不能从五个 shape 推导统一行数阈值；也未实测自动 JIT/轻 backend 的准备、转移与内存净成本。

仅保留三个未来研究优先点，不在本阶段执行：

- 若冷编译队列是实际瓶颈，研究准备与执行分离；若不能改善端到端 CPU/短任务尾部，停止。
- 若长扫描 IR CPU 是实际瓶颈，用更稳定短任务归因验证表达式表示/分派假设；若净收益在噪声内或生命周期成本抵消，停止。
- 若 tiny cold native 确有需求，再验证低准备成本 backend；若无法优于现有显式 IR/native，停止，不先建自动 tiering。

## 证据与验证

- [完整测量边界、表格、CLIF、复现命令](stage-strategy-experiment.md)
- [raw、identity、候选 patch、测试日志](../bench/results/i-stage/)
- [实验记录及历史负结果](experiments.md)，[H 混合负载](average-cost-experiment.md)
- `python3 tools/summarize_stage.py` 可重算；主控只读调用 summarize，三份结果与已提交 summary 完全一致。
- macOS all-features 与真实 Linux fixture-reference 各 61 项测试通过；Linux binary hash 回读后实际执行，VM 恢复 off。strict core clippy/fmt 通过，bench 仅保留既有两项明确 lint 例外。
- 唯一成功独立审查固定 d8f8d99：原生 Hermes delegate 返回 gpt-6-luna/openai-codex，无阻塞问题，重跑 macOS release all-features 通过。先前 Codex CLI 同型号 HTTP400 没有执行审查。最终仅补文档和证据表述；发布 SHA/远端回读写入任务交接。
