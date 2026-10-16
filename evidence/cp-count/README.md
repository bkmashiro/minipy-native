# CP：筛选计数 SIMD 准入与阶段取舍

结论：撤回唯一 SIMD 候选，defer SIMD 准入，停止 CP。最终 `src/`、`tests/`、Cargo 与已独立审阅的 CO `71582f7bdb9408d62a1838f2face9e0b24f21492` 完全相同，保留标量实验分支，不合入 main。不是证明 NEON 更慢，也不把本轮漂移下的回退百分比当作因果估计。

## 为什么做，以及唯一实现

CO 在两个既有 signed-i64 字段的窄纯形状上，确定删除逐行 tape dispatch，较大合成完整请求有约2倍信号，足以支持一次有界准入实验。复用 CO 的实际 scalar binary `co-b` 作为本次 A，逐文件核对其 source manifest 与 `71582f7`，不重建或混入 PGO。该正式二进制的两个 kernel 各20指令/80 B，只有 scalar load/cmp/checked adds/b.vs，没有向量寄存器。

唯一候选 B 先用安全 Rust `iter().filter(...).count()` 对已经预留 fuel 的纯 immutable chunk 统计 hits，然后 checked_add 到原 count。QuoteBuffer 的导入上界1,000,000行，slice hit count 转 i64 不溢出。不提前更新外部状态；checked_add 失败时重放原逐行 checked loop，保留先前成功更新的 count 和首个失败 row offset。只读预扫描没有 Host effect、row 对象构造或新增取消边界，不改变 fuel/index/chunk/yield、权限、owner 和 stdout 合同。空输入、零命中、尾部与字段 signed 比较都保持。

无需手写 intrinsics：LLVM 实际生成了 NEON，故没有追加另一份手写版本、CPU dispatch、目标特性、循环展开参数或 inline 搜索。无 JIT kernel 编译、额外 descriptor/IR/heap/task-state。CO 的40 B内联 descriptor、48 B Op、识别与 fallback 原封不动，debug-runtime 仍不融合。这里没有乘加 kernel、通用 schema、Tool ID/source 字符串特化、缓存、新后端或 PGO。

## 实际机器码及固定代价

Apple M4 / macOS26.4.1 arm64 / Rust1.96.0，Cargo release debug=1，默认 target features。`machine.json.gz` 是两份正式计时 binary 的全部实际 linked tape 函数，SHA 与 raw manifests 对齐，不是只数 wrapper 或从 Rust 外观判断向量化。

B 两个 kernel 均78指令/312 B。长度≥9时走8行/128 B的向量主体，四组 signed `cmgt.2d`、`sub.2d` 计数与 `addp.2d` 归约；按16 B row stride加载，价格/数量起始偏移0/8，LLVM 用 vector load 和 lane load 组合。剩余1–8行用 signed scalar compare/cinc，长度≤8全走标量。总数 checked adds/b.vs，失败跳原逐行 checked replay。安全 slice 迭代由编译器处理边界，无手写越界 load 或 padding 假设。

| 实际 linked 代价 | A scalar | B LLVM NEON |
|---|---:|---:|
| 两个 kernel bytes | 160 | 624 |
| wrapper + run bytes | 8084 | 8084 |
| 全部 linked tape text bytes | 14852 | 15304 |
| BT 测试 binary bytes | 10736320 | 10737360 |

linked tape 净增452 B，kernel 增464 B，其他 lowering 编码合计少12 B。测试 binary 增1040 B，包含链接/调试材料，不是服务 RSS。wrapper保留80+4096 B，run160 B，两个kernel都不建stack frame；这个同步调用路径预留4336 B，与A相同。没有新的生产布局/分配；未重测 allocator census、RSS、PMU 或 future layout。CO 的布局测量仅作为原基线，不包装成新的内存实测。

## 唯一一对完整请求 A/B + exact-A A/A

原 BT registered entry、相同 workload adapter/limits/schema、原计时边界；source/expected 构造和进程启动不计入请求，prepare + 实际执行/生产者构造导入 + code/output/临时Host清理全部计入。六个 opt-in count case 沿用 CO：7行、65537行，价格/数量混合命中，价格大输入另有零/全部命中；每请求改变初始count。未匹配 scalar/quote/ready 作控制，双后端均保留。

预先固定每cell每进程12正式请求、一轮ABBA，sample0排除；A/B与exact-A A/A各72进程/864请求，总144进程/1728正式请求。全部exit0且精确status/stdout断言通过。没有追加轮数、剔除慢进程、挑选B2、改变数据或根据结果改候选。

表中是进程中位数的臂中位数比；描述本轮观测，不给无回归界。A/A不是噪声上限。

| case | tape A→B µs | tape B/A | tape A/A | native B/A | native A/A |
|---|---:|---:|---:|---:|---:|
| price small | 16.240→47.438 | 2.92107 | .91987 | .89016 | .99631 |
| quantity small | 16.156→24.708 | 1.52933 | 1.01252 | .97470 | 1.01019 |
| price large | 202.865→318.208 | 1.56858 | .99168 | .99977 | 1.00307 |
| quantity large | 202.282→238.448 | 1.17879 | 1.01101 | 1.00135 | .98772 |
| none | 201.084→215.177 | 1.07009 | 1.00244 | 1.02547 | 1.00961 |
| all | 207.084→202.688 | .97877 | 1.00046 | .97772 | .99339 |
| scalar | 3.417→3.844 | 1.12490 | .98782 | 1.03018 | .98399 |
| quote unmatched | 15.386→16.157 | 1.05011 | 1.01289 | .98353 | 1.00356 |
| ready unmatched | 13.927→14.365 | 1.03143 | 1.03685 | .98618 | .98034 |

B1/B2 price small为78.188/16.688 µs，price large为445.167/191.250 µs；相应A0/A3 price large为206.104/199.625 µs。B进程明显漂移。B2的四个大输入均约189–192 µs，仅挑B2可得到较好方向，但不能代替预定整对结果。不变native price small也有约−11%差异。冷启动、安全扫描、调度等可能解释尚未验证，不能把它们当作已确诊原因。小输入走标量并不意味着完整请求代价必定不变。

startup fixture setup中位AB/AA分别75.291/75.042 µs，独立于请求，不是进程启动耗时。

## CPU 记录与局限

采集前最后两次有效 top delta idle96.60%/97.40%。1Hz期间完整有效delta idle96.96%/92.67%/89.15%/92.24%，未观察到≥50%单核的无关进程。首个累计snapshot不当interval delta，末尾不完整snapshot仅保留文本。开始前和结束后各留2.2s使有效区间覆盖短campaign；按前一snapshot时间近似对齐AB为sample2/3，AA为3/4，原文和receipt timestamps均保存。

工程guard为pre两次idle≥90%、during每个有效delta idle≥85%且无关进程<50%单核，首次不满足则只终止自身采集进程组，不杀其他进程，不重做。以上均达标。top列出8个主要进程，存在XprotectService、airportd和VM等活动；不是整机独占，也不能排除每个微秒级请求的抢占、频率、page-in或安全扫描。不能从本次idle反推CO/CN历史环境，也不与旧数据池化。

## 正确性、恢复与证据重放

候选完整release default/tool-json/tool-json+debug suites实际80/131/139项全通过；三套严格all-target Clippy、stable fmt、9项BT Python tests、diff check通过，完整stdout/stderr在 `verification.json.gz`。新增直接kernel测试对两字段、长度0..33/255/256/257/4095/4096、四种subslice offset、五阈值、七初始count共10,920组合，对比独立逐行reference的结果、首个overflow offset和部分count；涵盖MIN/MAX与向量入口/尾部。原CO端到端测试继续验证fuel不足、顺序effect、取消/owner、JSON跨chunk和形状拒绝。新测试随撤回代码仅留在 `candidate.patch`，不残留死路径。

先保存原候选patch，逆向apply恢复后，逐source SHA与CO一致。最初一次正式build误信环境说明的TMPDIR，输出落在实际macOS `/var/folders/.../T/`，下个objdump因在Hermes scratch找不到而失败；随后将两文件移到Hermes scratch并显式设TMPDIR。没有重做该初版build；加入test-only验证并fmt后只做一次最终正式build `cp-simd`，之后所有计时和machine证据均用它。初版 `cp-b` 未采样，不参与比较。没有失败的计时轮或候选参数搜索。

`collector.py.gz` 保存采集时脚本原文并匹配collection中的SHA；之后只加强只读replay，使其从base Git blob + candidate.patch重建B，验证恢复后的A源码、全部manifests/registered entry/driver、CPU原文、144进程/1728请求、实际linked代码和测试记录。派生summary重放必须逐字节相同。

```
python3 tools/cp_count_simd.py replay
python3 -m unittest discover -s tools -p test_cp_count_simd.py -v
python3 -m unittest discover -s tools -p test_perf_workbench.py -v
```

重建而非默认执行的新采集：从CO71582f7构建A，apply `candidate.patch`构建B，命令均为 `RUSTUP_AUTO_INSTALL=0 python3 tools/perf_workbench.py build --output NEW_PATH`。固定 `--rounds 1 --reps 12 --cases count_price_small count_price_large count_quantity_small count_quantity_large count_none count_all scalar quote ready`，A/B和A/A分开，新raw文件名，保留CPU guard。`collect`拒绝覆盖原证据；本卡已停止，不自动重采。formal binaries/sidecars留原scratch，不进仓库。

## 阶段取舍和下一方向

- 标量：keep CO已审阅窄候选在独立实验分支，大输入净值和确定dispatch删除足以保留研究结果；小输入和真实Host通用收益仍未建立，不擅自合main。
- SIMD：revert本次实现，defer准入。安全Rust可直接得到NEON，不需要intrinsics框架，但本轮整体未建立净收益，增加text/overflow双遍扫描没有充分理由进入生产。没有宣称优化空间耗尽。
- PGO：沿用CR签名 `c58cea8` 的固定低负载复核，native holdout B/A约.724–.844，保留正向研究信号、defer默认生产启用；不把其百分比乘到CO/CP，也不在CP重训练或变更PGO结论。
- 下一方向建议仅为：用户确定代表性真实Host新源码及输入分布后，再判断窄count形状的实际占比与完整请求成本，并为PGO建立可维护、与holdout分离的训练集。本卡不自动采集、扩大campaign、重开CG适配或重写后端。

所有构建/测试/反汇编/计时复用现有外盘target、minipy-perf-build.lock、单job、已装1.96.0 offline/locked；Clippy/fmt用已装stable。未建worktree/target、下载依赖、启用CI、修改网站/全局系统或杀无关进程。没有Linux/Wasm/Monty/真实Code Mode、RSS或吞吐新结论。main与CO/CR refs保持原值，CP证据在独立分支提交供原卡review。
