# MiniPy 性能与下一阶段设计审阅入口

本材料固定生产源码 `20e454ab36a55283f9c475c186f520a948d81571`。只新增审阅材料，不修改运行时。schema 修复 BD 仍在进行，本文不预判其结果。仓库早期 README/design/roadmap 含历史阶段叙述；当前支持范围以源码、测试及 AS 集成记录为准。

## 当前代码与实验边界

- [生产源码](https://github.com/bkmashiro/minipy-native/tree/20e454ab36a55283f9c475c186f520a948d81571)：共享前端、typed IR、紧凑 tape、Cranelift；Tool/JSON/string/HTTP 已合入，保留 opt-in。
- [AY 实验快照](https://github.com/bkmashiro/minipy-native/tree/446a8eba1c8d1bf27a48edf9980c00e68005b1f2)：固定宽度 schema 记录遍历、Quote/Record 扫描准备期拆分。未集成，旧 Quote tape 的一次配对 warm 仍约 +50.7%，原因未确认。读该版本 docs/experiments.md 的 AY 段及其原始证据路径。
- [BE 实验快照](https://github.com/bkmashiro/minipy-native/tree/faef68fecebc940ee02cf1d8bdf146c837285d1e)：JoinHandle 候选已撤回，生产源码相对其 main 基线无差异。复审批准证据，不批准性能修复。

## 按问题阅读

### 编译、解释与执行

- [紧凑解释器实现及实验](https://github.com/bkmashiro/minipy-native/blob/20e454ab36a55283f9c475c186f520a948d81571/docs/compact-interpreter-experiment.md)
- [编译分段诊断](https://github.com/bkmashiro/minipy-native/blob/20e454ab36a55283f9c475c186f520a948d81571/docs/compiler-profile.md)
- [历史优化与撤回记录](https://github.com/bkmashiro/minipy-native/blob/20e454ab36a55283f9c475c186f520a948d81571/docs/optimization-roadmap.md)
- 源码：src/frontend.rs、src/ir.rs、src/interpreter.rs、src/interpreter/tape.rs、src/codegen.rs、src/runtime.rs。
- 测试：tests/ 下实际语义、错误顺序、取消、owner 生命周期用例。

### 现成真实 IR 材料

以下都是仓库已有的编译器实际输出，**对应历史实验版本，不是新生成的当前 IR**。各目录的实验说明/身份记录决定其来源。不要直接拿它们推算当前 CPU 占比。

- [E 六类程序 pre/post CLIF](https://github.com/bkmashiro/minipy-native/tree/20e454ab36a55283f9c475c186f520a948d81571/bench/results/e-compiler-profile/ir)：empty、scalar、scan、weighted、two_scan、legacy。
- [F 条件 lowering 前后 CLIF](https://github.com/bkmashiro/minipy-native/tree/20e454ab36a55283f9c475c186f520a948d81571/bench/results/f-predicate/ir)：baseline/candidate 的 scalar、weighted、nested。
- [I 基线 CLIF](https://github.com/bkmashiro/minipy-native/blob/20e454ab36a55283f9c475c186f520a948d81571/bench/results/i-stage/base-clif.txt) 和 [撤回候选 CLIF](https://github.com/bkmashiro/minipy-native/blob/20e454ab36a55283f9c475c186f520a948d81571/bench/results/i-stage/native-candidate-clif.txt)。
- 当前 tape 定义及生成规则直接看 src/interpreter/tape.rs；没有把手写伪指令当成实际 dump。当前 CLI 的 --ir 为 Cranelift 输出，不是 tape dump。

### JSON、Tool 与 HTTP

- [AS 集成功能、成本及 owner 契约](https://github.com/bkmashiro/minipy-native/blob/20e454ab36a55283f9c475c186f520a948d81571/docs/as-integration.md)
- [JSON 设计](https://github.com/bkmashiro/minipy-native/blob/20e454ab36a55283f9c475c186f520a948d81571/docs/tool-json-view.md) / [测量](https://github.com/bkmashiro/minipy-native/blob/20e454ab36a55283f9c475c186f520a948d81571/docs/tool-json-results.md)
- [AL 调度成本归因](https://github.com/bkmashiro/minipy-native/blob/20e454ab36a55283f9c475c186f520a948d81571/docs/al-io-results.md)
- [BE 原始数据、候选 patch、归约与进程 CPU](https://github.com/bkmashiro/minipy-native/tree/faef68fecebc940ee02cf1d8bdf146c837285d1e/evidence/be-http-cost)
- 本目录 be-summary.json、be-process-cpu.json 是 BE 对应文件的逐字复制，方便直接阅读；原文件与脚本仍以上方固定版本为准。
- 源码：src/tool.rs、src/json_view.rs、src/http_provider.rs、src/provider.rs，结合双后端调用位置。

BE 重要观察：三次普通 yield 约 1.33µs，detached+yields 约 13.43µs（见 summary 的具体控制实验）。不能把差额全部断言为 oneshot 成本。JoinHandle 替换没有跨路径稳定净收益；mixed tape source-cold 个别收益不能抹去其他单元/CPU的结果。process-cpu 是独立进程总 CPU，包含 setup，不能当成单请求 CPU。

### 实际新源码工况与需求

- [BC 五路新代码请求](https://github.com/bkmashiro/minipy-native/blob/20e454ab36a55283f9c475c186f520a948d81571/docs/bc-fresh-source-benchmark.md)：MiniPy 两路、Monty、CPython spawn/prefork；每 shape 30 个语义变体、五路同源。数据生成和表示转换计入请求，比较的是各自完整实现路径。
- [BC raw 与 summary](https://github.com/bkmashiro/minipy-native/tree/20e454ab36a55283f9c475c186f520a948d81571/bench/results/bc-wsl)
- [高频操作/kernel 候选](https://github.com/bkmashiro/minipy-native/blob/20e454ab36a55283f9c475c186f520a948d81571/docs/ao-kernel-selection.md)：基于本地轨迹的聚合，出现频率不等于运行 CPU 热点；原始私人轨迹不公开。
- [持续容量实验](https://github.com/bkmashiro/minipy-native/blob/20e454ab36a55283f9c475c186f520a948d81571/docs/af-capacity-benchmark.md)：不同冻结版本与资源边界，不能同 BC 直接拼接计算收益。

## 当前需要回答的三个问题

1. schema 已静态绑定，为什么未使用 record 的旧程序仍明显变慢？区分运行时额外工作、指令/状态布局与编译器代码生成效应、以及测量混杂。
2. 借用 JSON + HTTP 的价值如何兑现，哪些成本能删除，哪些是异步和取消语义的必要成本？
3. 一次性源码下，解释器、预编译 kernel、整程序 native、局部按需编译如何选择，才会改善整体平均成本而非单一大循环成绩？

完整问题与输出要求见 [prompt.md](prompt.md)。本轮材料整理未启动 Rust 编译或新性能 campaign。
