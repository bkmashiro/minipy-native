# CJ：省略已证明全零的 buffer liveness

决定：保留这一最小候选，结束本轮。七类 zero-buffer tape prepare 实测各少一次 allocation 和 32–72 B backing，且不再执行 buffer transfer pass。非零 buffer、JSON liveness、native lowering 保持。请求延迟证据混合，不宣称通用提速、无回归界、CPU cache/RSS 收益或真实 Code Mode 支持扩大。不追加样本追逐结论，不启用第二布局候选。

## 实现与不变量

基线 A 为 `e1ee1af7cb7e016504d4dac56e1e05199799b7c2`。B 只改变 `src/interpreter.rs`、`src/interpreter/debug.rs` 及 buffer oracle 测试。精确差异保存在 `candidate.patch`，reducer 用 Git blob 加 patch 重建所有 measured source hashes，而不是信任采集时两臂相同的 HEAD 标签。

Admission 为每个 Quote buffer 分配 slot；`program.buffers == 0` 时，合法 tape 不可能含引用 Quote buffer 的 Load/Scan/BufferLen，因此旧 buffer mask 在所有 PC 均为零。prepare 用无 backing 的空 Vec 表示该情况，省掉原 `ops.len()+1` 个 u64 的初始化及逆序 transfer pass。`buffer_mask(pc)` 以独立的 admitted `buffers` 判别 zero case，非零 case 保留 `live[pc]` 的 bounds 检查，不用 `get(...).unwrap_or(0)` 掩盖错误。

同步/异步 production 和 debug-runtime 的全部 buffer-mask 读点走同一方法。JSON roots 有自己独立的 `json_liveness`，未因 Quote buffer 为零而省略。预算、授权、等待、取消、输出错误顺序和 owner release 仍由原路径负责。

逐 PC oracle 覆盖 scalar、branch、record、直线/分支/循环 buffer，另覆盖 zero-buffer JSON、string wait 及 mixed JSON+Quote loop。新增断言检查 zero capacity、zero buffer visits；非零仍逐 PC 等价于独立 pre-CD fixed-point oracle。显式非法 PC 的非零 unit 必须 panic，仅测试内部 sealed-state 错误，不改变普通用户错误契约。完整 default 和 tool-json/debug-runtime suites 保留 owner、取消、预算、authority、overflow 和错误顺序的现有回归覆盖。

## 单轮请求 campaign

未修改 `perf_workbench` adapter/driver，复用 BT registered environment 和 CF Notify-gated waiting case。七项为 scalar、branch、ready、escaped_join、detach_wait、quote、madd_loop，双后端。A/B 与 exact-A A/A 各两轮 ABBA，每进程 sample 0 排除、24 measured samples；总计 224 个进程、5,376 measured requests，全部 exit 0、精确 status/stdout/gate 断言通过。无缺失、失败或丢弃的正式观测。

每个请求计时从新 source prepare 开始，包括实际执行、producer 构造导入、effect 调度、精确断言和 code/output/临时 Host 清理。source/expected 构造、resident environment/runtime 启动在计时外。没有 prepared code/result 复用。正式二进制没有 allocator、phase clocks 或 compiler-profile；下面 allocation census 是单独 test-only 构建，不能与正式时间混合。

B:A 为各进程中位数的臂中位数之比，仅描述性结果：

| backend / case | A µs | B µs | A/B B:A | exact-A A/A B:A |
|---|---:|---:|---:|---:|
| tape scalar | 3.021 | 2.886 | 0.95515 | 1.01390 |
| tape branch | 7.115 | 7.635 | 1.07316 | 1.01485 |
| tape ready | 11.990 | 12.083 | 1.00782 | 0.99913 |
| tape escaped_join | 12.479 | 12.198 | 0.97748 | 0.98454 |
| tape detach_wait | 76.688 | 76.855 | 1.00217 | 0.98225 |
| tape quote | 12.354 | 12.855 | 1.04051 | 0.98735 |
| tape madd_loop | 15.250 | 14.573 | 0.95559 | 0.98368 |
| native scalar | 43.365 | 44.406 | 1.02401 | 1.00246 |
| native branch | 66.625 | 64.052 | 0.96138 | 1.00844 |
| native ready | 81.573 | 81.115 | 0.99438 | 0.99237 |
| native escaped_join | 77.052 | 75.615 | 0.98135 | 0.98853 |
| native detach_wait | 154.083 | 154.354 | 1.00176 | 1.00319 |
| native quote | 106.167 | 106.000 | 0.99843 | 1.01849 |
| native madd_loop | 116.136 | 114.594 | 0.98672 | 0.98039 |

最早 tape scalar 的两个进程中位数为 A 12.729 / B 9.625 µs，后续多数约 3 µs；完整保留早期漂移。tape branch +7.3%、quote +4.1% 是必须报告的负向描述性结果，A/A 的对应变化较小；不能因此直接证明因果回归，也不能用“A/A 噪声”抹去。相邻 madd_loop 则 -4.4%，native 实现完全不变而仍有约 -3.9% 至 +2.4% 的差异。这个预算内没有一致的延迟净收益或确定回归结论。scalar/branch 的 Done 前不会读取 buffer mask，不能把 branch 差异归为新增边界判断。保留依据是直接删除已证明无用的 work/backing，而非该表的提速估计。

## 独立 allocation、布局与实际代码代价

`diagnostic.patch` 临时接入原 CI 九类 fixture 与新的 `allocation.rs`。每类每后端先 warmup，再独立采三次 prepare+drop allocation census，三次值完全一致。只在当前 test thread 计数 alloc/alloc_zeroed/realloc 调用和 requested bytes；不计 allocator rounding、峰值活跃内存或 RSS，native compiler scratch 可复用。environment 构造及报告格式化不在 census 内。

| tape case | allocation A→B | requested/backing bytes 减少 |
|---|---:|---:|
| scalar | 32→31 | 32 |
| bound_tree | 58→57 | 56 |
| madd_short | 57→56 | 48 |
| branch | 84→83 | 72 |
| ready | 116→115 | 64 |
| joined_sink | 127→126 | 56 |
| async_string | 102→101 | 56 |
| quote | 99→99 | 0 |
| madd_loop | 119→119 | 0 |

七项 live capacity 归零，prepared_bytes 同量下降；非零 buffer 的 ops、mask length/capacity、prepared bytes 不变。所有九类 native allocation/requested bytes、生成 code_bytes 不变。双后端重新执行所有九类，精确结果均通过。

Op/Operand/Dest 仍为 56/16/16 B，Unit/State 为 112/312 B，native Frame 320 B、同步 scratch 4096 B。本 Provider 的 execute future 仍 tape/native 928/664 B。小 Dest 镜像只是复用原 CI 的第二候选诊断，没有生产布局更改。

不能把访问判断当作零成本：实际 AArch64 async `execute_script` dispatcher 为 1,826→1,835 指令（7,304→7,340 B，+36 B），不是整个 binary 或动态执行指令数。B 在 effect/yield boundary 的 mask 读取前保留 `ldr ... #0x60` 和 `cbz/cbnz`，非零分支仍有 `cmp`/`b.hs` bounds exit 及 mask load。例：B function offset `+0xa24` 的 admitted-buffer load / `+0xa28` zero branch，之后 `+0xa38` bounds branch、`+0xa40` mask load。这项成本不进入 tape 的逐 Op/逐 Quote-row dispatcher；不主张每请求只执行一次，也不从静态指令数推导 cache misses 或吞吐。完整反汇编保留在 compressed diagnostic 的每臂末条 command stdout。

综合判断：确定少一个 prepare allocation、一次 pass 与 32–72 B backing，交换边界读取上的分支和 +36 B 这一个实例的 async code，维持一个很小的内部表示，值得保留。收益很小且 workload 适配限制仍在，此处停止，不延伸到缓存、新编码、优化器或大后端。

## 身份、验证与复现

Apple M4 / macOS ARM64、Rust 1.96.0，`RUSTUP_AUTO_INSTALL=0`、`--offline --locked --release`、单 build job。既有外置 `/Volumes/M.2/caches/minipy-native-target` 挂载且 `target` 保持 symlink。构建、计时和 Rust 验证共用 `~/.hermes/cache/scratch/minipy-perf-build.lock` fcntl lock，无新 worktree/target/dependency/download/CI。native latency 是未改路径的控制，不是新 native 优化。没有 Linux/Wasm/Monty/生产 Code Mode 性能结论。

`verification.json` 保存完整 release suites、CC waiting fixture、严格 all-target Clippy、fmt、diff check 的实际命令和 exit 0。另九个 Python workbench contract tests 通过。本轮 liveness test console 已确认 zero buffer visits=0、nonzero straight/branch=7/10、mixed loop=36，独立 JSON pass=5/10/7/36/8；测试断言在最终源码内。

工程诊断失败如实保留说明：首个 patch 因同文多处匹配原子拒绝，未改文件，补唯一上下文后成功；导出 patch 的 terminal stdout 没有末尾 newline，首次 `git apply` 拒绝，修正 patch 后再跑。首次 allocation overlay 误用了不存在的 native `prepared_bytes`，构建失败；删除该指标、保留 native null，不为方便新增 API。随后第一次符号筛选用未生成的 `run17` 命名失败，依据实际 symbol table 改抓 inlined `execute_script` async closure。两次失败均 finally 恢复 B、未产生正式观测；最终诊断六条 command/arm 均 exit 0。失败诊断文件没有有用测量，清理，未计入成功次数。

只读重放，不重跑计时：

```
python3 tools/cj_zero_reduce.py
python3 -m unittest discover -s tools -p test_perf_workbench.py -v
```

新采集须选新 raw 路径，避免 overwrite。干净 A checkout 先 build A，再 apply `candidate.patch` build B；不得携带 diagnostic overlay 构建正式 binary。

```
python3 tools/perf_workbench.py build --output /Users/yuzhe/.hermes/cache/scratch/cj-a-new
# apply candidate.patch, then:
python3 tools/perf_workbench.py build --output /Users/yuzhe/.hermes/cache/scratch/cj-b-new
python3 tools/perf_workbench.py run --baseline /Users/yuzhe/.hermes/cache/scratch/cj-a-new --candidate /Users/yuzhe/.hermes/cache/scratch/cj-b-new --cases scalar branch ready escaped_join detach_wait quote madd_loop --rounds 2 --reps 24 --output NEW_AB.jsonl.gz
python3 tools/perf_workbench.py run --baseline /Users/yuzhe/.hermes/cache/scratch/cj-a-new --aa --cases scalar branch ready escaped_join detach_wait quote madd_loop --rounds 2 --reps 24 --output NEW_AA.jsonl.gz
```

`tools/cj_zero_probe.py` 从 B 临时 reverse 精确 candidate patch 获得 A，再分别临时 apply 同一诊断 overlay；finally 恢复 B，并逐源码 SHA-256 核对。这个采集器默认 output 已存在时拒绝；复现时改用新的 output 路径，不删除原 raw。`summary.json` 全部由 reducer 重建，验证正式两臂每个 source、共同 adapter/driver、diagnostic inputs、每 case/sample 数和 allocation/size invariant。

BT/BX/BY/CB 的负结果、BV defer、CG 真实 workload 适配 stop 不变。最终审阅另见 `review.md`。
