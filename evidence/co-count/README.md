# CO：单一筛选计数标量批量 kernel

取舍：保留独立分支上的一个窄形状候选，停止 CO，不合入 main。确定减少逐行 tape 工作；大输入完整请求给出足够继续评估的信号，小输入没有可靠净收益保证。CP 可以以当前实际标量机器码为基线判断 SIMD 是否值得，不能把 PGO、缓存或新后端混入比较。

## 边界与实现

基线 `44212e00e9161b479b6a367409087e82d3520190`，开始 clean；生产源码/Cargo release 配置与获准的 `907eb40` 相同，CN 只增加实验材料。正式 A/B 都未用 PGO flags，没有训练或用户请求期 kernel 编译。

源码先核对 frontend：只支持已加载 Quote 的两个固定 signed-i64 字段 `unit_price_cents` / `quantity`，嵌套循环和循环后 row 名引用不被接纳。没有引入通用 records/schema。唯一匹配形状：整个循环体恰好是 `if row.FIELD > THRESHOLD: count = count + 1`，无 else；THRESHOLD 只能是 i64 literal 或与 count 不同的 scalar local。比较方向、额外算术/statement/effect、short-circuit、不同增量、count 自身作为阈值一律走原 tape。字段及阈值由 admitted IR 绑定，不看源码字符串、Host/Tool ID 或预期答案。

Lowering 用 `Start + Count` 替换原 Scan/compare/Add/Jump/Next 区段，消费并释放原 IR。40 B descriptor 内联在原 48 B Op 中，无 descriptor heap allocation/旁表/IR 备份/额外 per-task 字段。原循环仍是所有未匹配程序的唯一 fallback；不增加另一个执行器。`debug-runtime` feature 保留原 lowering，不融合，保持逐 Op read/write/row trace，生产 release 不带这个 feature。这个明确的诊断模式取舍不代表已验证“融合单步执行”。

每 chunk 从 sealed immutable owner 借用原 row slice，不复制行。一次读取阈值并按 Field 选择两个静态标量实例之一。kernel 逐行 signed compare，命中时 checked_add(1)，在首个溢出行返回 offset；此前成功更新的 count、index、chunk_left 和 row 保持顺序，之后不执行 print/record。空输入不耗 fuel；其余仍按 `min(rows_left, chunk, remaining)` 预扣 fuel，最后短 chunk 不额外 yield，只有仍有行才经原 fairness/cancel boundary 返回。loop 后 count、threshold、其他 scalar 和后续多个 loop 保持。buffer liveness 把 Count 当作 buffer 使用，跨 yield 继续保留 owner，取消后释放。JSON liveness 的普通 successor transfer 足够，实测 live JSON string 跨 count chunks 保持。

## 确定的工作量、布局与真实机器码

Apple M4 / macOS26.4.1 arm64 / Rust1.96.0。独立 test-only probe 与正式计时分开；600 rows，正负混合，两个字段各300命中，精确 stdout/status 与 native 均断言。

| 项目 | 原 tape A | 候选 B |
|---|---:|---:|
| Op / Operand bytes | 48 / 16 | 48 / 16 |
| Unit / State bytes | 112 / 312 | 112 / 312 |
| 同步 scratch bytes | 4096 | 4096 |
| 本具体 TestProvider tape/native future bytes | 928 / 664 | 928 / 664 |
| 两个匹配形状 ops / capacity | 11 / 16 | 7 / 8 |
| used tape / backing bytes | 528 / 768 | 336 / 384 |
| buffer live capacity / backing bytes | 12 / 96 | 8 / 64 |
| `prepared_bytes()` 逻辑估算 | 976 | 560 |
| chunk7 dispatch / fuel reservations / yields | 2407 / 86 / 85 | 93 / 86 / 85 |
| chunk256 dispatch / fuel reservations / yields | 2407 / 3 / 2 | 10 / 3 / 2 |

不是 allocator census 或 RSS。prepared 减416 B = tape backing384 + live backing32；没有声称所有程序少 allocation。自引用阈值(alias)和普通求和(unmatched)的 Op 数、capacity、prepared 与 dispatch/fuel/yield 逐项完全不变。

`machine.json.gz` 来自正式 A/B binary，SHA 与 timing manifests 相同。每个 `scalar_count` 20条/80 B：`ldr x`、`cmp`、`b.le`、`adds`、`b.vs`、scalar store，row stride16，两个字段 load offset0/8。没有 NEON/SIMD/vector register 指令。没有用“Rust 源看起来 scalar”代替检查实际代码。

需报告的全局代价：LLVM 把 B 的 `run` 从 wrapper 分离，不能只数34条 wrapper 便声称缩小。原 dispatcher7500 B → wrapper136 + run7948 =8084 B（+584），两个 kernel共160 B，Lower::statements2228→2600 B（+372）。所有实际 linked tape text 合计13736→14852 B（+1116），BT测试binary10719024→10736320 B（+17296，含调试/链接材料，不是服务resident size）。同步调用栈预留 A=96+4096+64=4256 B，B=80+4096+160=4336 B（+80），两个 kernel自身不建stack frame。未来 future大小不变不表示同步栈也不变；多一个 wrapper→run call，每个非空chunk一个 scalar kernel call。不继续通过 inline 属性搜索机器码。

## 完整新源码请求：两次独立、小规模采集

沿用 BT registered entry、schema/limits/driver/timing boundary。只为六个 opt-in count case 增加负batch对应的有界生产者数据，两臂 adapter 一致；原 BT case 的256行保持。7行小输入与65537行大输入，每请求不同初始count。价格/数量混合分布，另含大价格全部命中/全部不命中。完整计入每份源码 prepare、实际执行/生产者构造导入、code/output/临时Host清理；source/expected构造、进程启动不在请求计时内。没有预备代码/结果cache、隐藏预扫描或正式路径插桩。

每对 A/B + exact-A A/A 各一轮 ABBA，每cell每进程12正式请求，sample0按原契约排除；九类双后端每个raw72进程/864正式请求。原对和带监控替代对各144进程/1728请求，共288进程/3456请求，全进程exit0且status/stdout精确断言通过。两对不池化，不挑子集，不增加轮数，也没有据结果修改候选。

第一对 `ab/aa.jsonl.gz` 未保存期间CPU记录，回读卡片新监控要求后明确降级为未控环境观测。以下先给同binary、同样本计划的唯一替代对 `monitored-ab/monitored-aa`：进程中位数的臂中位数比值，描述性而非精确因果估计。

| case | tape A→B µs | tape B/A | exact-A tape A/A | native B/A | exact-A native A/A |
|---|---:|---:|---:|---:|---:|
| price small | 16.604→16.615 | 1.00063 | .95799 | .98637 | 1.01065 |
| quantity small | 16.229→16.146 | .99487 | .98453 | .99660 | .99035 |
| price large | 478.542→201.511 | .42109 | 1.00833 | .98369 | .98505 |
| quantity large | 433.677→201.458 | .46453 | 1.00191 | .94159 | .98455 |
| none | 388.531→200.115 | .51505 | 1.00720 | .99192 | 1.00732 |
| all | 495.510→204.115 | .41193 | .97614 | .96951 | 1.01090 |
| scalar | 3.760→3.677 | .97786 | .91690 | 1.02627 | 1.00643 |
| quote unmatched | 14.323→14.333 | 1.00072 | .99857 | 1.04016 | .99763 |
| ready unmatched | 13.573→13.979 | 1.02991 | 1.03704 | 1.01111 | 1.01914 |

原未监控对也必须保留：large price/quantity/none/all B/A分别.46507/.50310/.55150/.44497；small price/quantity 为1.20180/1.09843，quote1.06486，ready1.01305，scalar.98026。原 price_small A0/B1/B2/A3 为60.042/75.354/16.834/16.667 µs，price_large A0/A3为975.229/478.146 µs，明显有时间漂移。不能用替代对的小输入接近1覆盖原回退观测，也不能用 A/A 当作噪声上限。native数量大输入替代对仍有−5.8%差异，未建立整机硬独占、因果净收益百分比或无回归界。

startup单列：原AB/AA与替代AB/AA的fixture setup中位分别75.333/72.896/75.146/74.021 µs；不是进程启动耗时或请求延迟。

CPU记录 `cpu.json.gz`：唯一替代对前top三个snapshot最后两次96.46%/97.14% idle，主要竞争VM约13%单核、无≥50%单核的无关进程。期间记录两snapshot：首次79.52% idle是新top进程累计snapshot，不能作这段delta；随后有效delta为87.74% idle，列出主要进程但未见≥50%无关竞争者。总采集约一秒，1Hz仅给粗粒度观测，不证明每个短请求无抢占。工程guard为pre末两次idle≥90%且无无关≥50%，during连续两次idle<85%或无关≥50%就终止自身采集，不杀无关进程。

采集器有一次明确失败：两个替代raw都已完整成功后，reader只在下个snapshot header到来时结算block，漏结EOF最后一个block，使`len(samples)>=2`后置断言失败。原CPU raw保留一条online评估和两份实际snapshot，未重写；reducer直接重读CPU文本，末次delta仍可核对。当前脚本已补EOF结算，未为修这个记录程序重做第三轮计时。这个限制降低“持续空闲”的证明力度，不影响raw请求数、二进制或正确性；不把两轮结果合并成更高置信估计。

## 正确性、失败记录与复现

最终完整release suites实际通过：default79、tool-json130、tool-json/debug-runtime138；三套严格all-target Clippy、stable fmt、9个Python BT tests和diff check均通过。融合模式与禁用融合debug模式分别执行。`tests/co_count.rs`覆盖两字段、signed extrema、空/短/尾部/600行、四种chunk、全部/无/混合命中、累计fuel不足和边界恰好结束、checked overflow先于后续effect、permission/buffer cap、跨chunk取消/弱owner释放、相同immutable unit独立复用、未匹配shape和多个loop，以及live JSON owner跨融合chunks。普通输入只得原有task/compile errors。

早期两个测试expectation修正：TestProvider record_total原本会checked加batch导致额外overflow，改用batch0使断言聚焦count；host scalar input immutable，改为mutable local验证形状拒绝。新增JSON fixture第一次用reserved Tool ID1被schema拒绝，改为101。首次Python suite发现opt-in roster期望尚未加入六个case，补足精确名单，保留原默认四case。所有失败在terminal会话或`verification-first/json-fixture`原始日志中；后续成功日志另存。初次JSON测试插入V4A上下文不唯一原子拒绝，重读后精确补上下文，无部分写入。首次reducer把十六进制编码`d2800000`误当`dN`寄存器，改为仅检查decoded instruction operands，未放松kernel机器码检查。

构建/诊断/测试/请求计时复用原`minipy-perf-build.lock`、已挂载外置target、`CARGO_BUILD_JOBS=1`、`RUSTUP_AUTO_INSTALL=0`、`cargo +1.96.0 --offline --locked --release`；Clippy/fmt用已装stable。无新worktree/target/依赖/下载/CI/网站更改。无PMU/RSS/allocator census、Linux/Wasm/Monty/真实Code Mode新结论；CG stop与其他已撤回候选不变。

只读证据重放（只重新生成派生summary，不build/计时）：

```
python3 tools/co_count_reduce.py
python3 -m unittest discover -s tools -p test_perf_workbench.py -v
```

Reducer从base Git blob + `adapter.patch`/`candidate.patch`逐项重建所有正式source hash，核对4份raw的binary identity/registered entry/driver、288进程和3456正式请求、diagnostic source/layout/future/dispatch/fuel/yield、完整实际linked tape反汇编和最终production hash。`diagnostic.json.gz`最初只选wrapper，`machine.json.gz`补齐LLVM自动分离的run和所有linked tape方法，报告使用后者，不静默覆盖前者。

新采集需新output/raw名字：基线+adapter先`perf_workbench.py build --output NEW_A`，apply candidate.patch再build NEW_B，原命令是`run --rounds 1 --reps 12 --cases count_price_small count_price_large count_quantity_small count_quantity_large count_none count_all scalar quote ready`，双后端默认，分别`--candidate NEW_B`及`--aa`。CPU guard/记录见`co_count_monitored.py`，诊断overlay逆向恢复见`co_count_collect.py`，真实machine选取见`co_count_machine.py`，验证见`co_count_verify.py`。采集脚本拒绝overwrite。正式两binary和sidecar暂留原scratch供固定审阅，非tracked artifact，不另复制进仓库。

最终推荐保留依据是：单个无额外heap/Op/future成本的窄形状，确定消除逐行解释器dispatch；两种字段及零/全/混合大输入均有完整请求约2倍量级信号，强于增加的约1.1KB tape text与80B同步栈。不是通用任意Python提速。main集成等待原卡独立审阅；CP仅在认可这个净值后判断SIMD，不实现新的通用框架，不提前启用PGO。

发布前 staged diff check 单列 literal `.patch` 文件：其中表示原源码空行的单个 context space 必须保留，否则破坏已记录patch hash/重建。非patch源码、测试、脚本及文档保持默认严格 `git diff --cached --check`；三个literal patch用精确source replay与临时apply/reverse验证，不把context space误当生产源码尾空白。
