# CK：tape 临时槽的有界实验

经固定提交独立审阅，撤回唯一候选，生产源码和测试逐字节恢复基线，结束此路线。
确实删掉了小程序的初始化写入，但没有建立完整请求净收益，同步栈保留量反而增加。

## 不变量与单一候选

基线 `049b8102509d2d687912f0898ef1eda63bb81b14`，开始时 clean 且远端相同。
`Lower::temp` 的保守硬上界来自 admission `MAX_NODES=512`，不表示有程序恰好需要512槽。
`next` 每 statement 重置，递归表达式的每个非叶值独占槽；生成 operands 后才发射计算/写入。
短路路径在相应 merge 前各写结果。If 条件在分支体复用槽前已消费；循环 body 的值不跨迭代。
Load/Record 在返回 boundary 前已读完参数，普通 Tool 参数已绑定为 locals。
Next 的 chunk yield 发生在 body 完成之后，恢复到 Scan；错误不会恢复到失败表达式。
故 production turn 不依赖上次 scratch，源代码和 CI 的实际机器码都确认每 turn 重新清零。
Debug `Step` 可以停在表达式中，必须保留 Frame scratch，不能照搬 production 生命周期。

唯一候选在 lowering 内累计高水位，无额外 traversal；prepared unit 增加一个 usize。
<=16 槽使用安全 `[0;16]`，否则保留 `[0;512]`，共用接受 slice 的 dispatcher。
16 是本轮固定工程阈值，未搜索最优值。没有 unsafe、堆分配、新逐op检查、执行器复制或语义变更。
原数组 bounds 检查变成 slice bounds 检查。Debug 仍持有原完整数组，两个 stepping specialization 不变。
无需依赖读前写证明来使内存读取安全，两条路径的整个可访问数组都初始化。

测试 oracle 沿 CFG 传播 definite-write intersection，在 effect 和每个可能 yield 的 Next 清空集合，
逐个断言测试图上所有 Temp 读已写，且所有引用落在 lowering 高水位内。它不在普通statement切换处
清空集合，可能接受沿用前一语句写入的读，不能独立证明所有语句的读前写；这一限制由review指出。
源码的statement生命周期分析仍成立，候选的内存安全也不依赖该oracle。literal、alias、短路、分支、循环和JSON
覆盖实测 0/1/2/3/7/15/31/127 槽。新执行回归用原前端和双后端跑直线、分支、深平衡表达式，
以及600 rows/chunk7（按执行规则推导85次yield，非独立计数）和真实 `yield_now` Tool 挂起；同时验证明确期望输出。
原 suites 继续覆盖 overflow、错误顺序、取消、fuel、authority、owner、JSON/string和debug逐步恢复。

## 实际代码与完整请求

来自正式 A/B 二进制的 AArch64 反汇编，未用 diagnostic binary 替代：
A 在入口调用 `_bzero(ptr,4096)`。B 的 `temp_slots<17` 分支使用四条 `stp q0,q0` 写128 B，
大路径仍调用同一 `_bzero(ptr,4096)`；indirect-symbol table 解析了实际 import，不能把 objdump 的
`dyld_stub_binder+...` 标签当成函数身份。小路径少3968 B初始化写入，未测CPU cache misses。
A 的初始化和dispatcher合并为7628 B；B wrapper192 B + 唯一共享dispatcher7400 B，共7592 B。
新wrapper增加一次阈值读取/判断、参数传递和call。A 同步栈预留4256 B，B wrapper4192 B +
callee176 B =4368 B，增加112 B；栈仍有probe store，不能将128 B清零范围称为128 B栈frame。
Unit 本体112→120 B；State312 B、Op56 B未变。具体Provider九类future仍tape/native928/664 B。
等待future不持有同步scratch。没有allocation census、RSS、PMU、吞吐或生产Code Mode收益结论。

复用未经修改的BT adapter/provider/limits/driver。A/B和exact-A A/A各一轮ABBA，五类双后端，
24 measured requests/process，sample0排除；共80进程/1920正式请求，全成功，无筛除或追加。
计时包含新源码prepare、真实执行、producer构造导入和资源回收；source/expected构造与resident环境启动在外。
下表是进程中位数的臂中位数比值，不是精确因果估计：

| case | tape A/B B:A | tape A/A B:A | native A/B B:A | native A/A B:A |
|---|---:|---:|---:|---:|
| scalar | .97750 | .92598 | 1.00263 | .98779 |
| branch | .99697 | .96872 | 1.00249 | 1.02306 |
| madd_short | .96751 | 1.00936 | .97568 | 1.05358 |
| madd_loop | 1.00873 | 1.01484 | .98033 | 1.02114 |
| detach_wait | 1.00050 | 1.01057 | 1.01047 | 1.02415 |

Scalar的A/A漂移大于候选差异，branch/wait没有可见净值，loop略慢；madd_short的局部差异不足以
抵消布局/调用成本并推广。native source未改，是漂移控制。未重做Monty/Linux/Wasm或扩展语料。
结论依据是小初始化写入换取额外元数据、调用和更大同步栈，完整请求证据弱，简单保留基线更合适。
不再用unsafe、分级数组泛化、特化Tool或继续调阈值挽救此候选。CI第二布局候选和CG stop结论不变。

## 复算与验证

Apple M4/macOS ARM64、Rust1.96.0；已挂载外置共享target，单job、禁用自动安装、offline/locked。
全部Cargo/build/测量共用原fcntl锁。无新target/worktree/依赖/下载/CI，临时formal binaries在审阅后删除。
`candidate.patch.gz`完整保存候选及测试，raw manifests逐源码hash由基线Git blob+patch重建。
`machine.json.gz`对应正式binary hashes，`layout.json.gz`是单独临时CI overlay，最后逐源码hash确认恢复。
完整default和tool-json/debug-runtime release suites、严格all-target Clippy、stable fmt、9 Python tests通过。
两次初始fixture错误（改变变量类型、loop内Tool）已修正，未放宽语言；首次reducer对新增文件的zero-line
hunk不能复用旧apply helper，改为显式新增文件重建后通过。失败记录保留，不含正式计时失败。

只读重放（summary是派生文件）：`python3 tools/ck_scratch_reduce.py`。
新采集：基线先用原 `perf_workbench.py build`，`gzip -dc candidate.patch.gz | git apply`再build B；用原driver
`run --cases scalar branch madd_short madd_loop detach_wait --rounds 1 --reps 24` 分别跑A/B和`--aa`。
`ck_scratch_machine.py`读取固定scratch中的ck-a/ck-b，`ck_scratch_layout.py`只在候选状态应用/恢复原CI overlay。
所有raw拒绝覆盖，重跑应改新输出路径；不可把新binary或raw冒充本轮。审阅记录另见 `review.md`。

最终撤回还移除了仅服务候选的oracle/执行回归，完整实现和测试仍在固定候选commit及压缩patch内。
回退后的完整release suites、严格Clippy/fmt和原Python tests另记录于 `final-verification.json`。
