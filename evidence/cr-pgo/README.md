# CR：固定 PGO 二进制的低负载复核

结论：保留 Rust-host PGO 实验与 native holdout 的一致正向信号，暂缓默认生产启用。旧 CN 保留为“未控环境、可能受争用影响但未证实”的观测；本次独立小复测不与旧组池化，不证明过去发生过干扰，也不支持“PGO 没有调节空间”。未改变训练、LTO、CPU flags、生产源码或 CO kernel。停止采样。

## 身份与时间核查

起点 clean main `820c79b`，证据分支 `exp/cr-pgo-recheck`。生产源码、Cargo manifest/lock、原 BT adapter 均与 CN 基线 `907eb40` 逐文件 SHA256 一致。为独立回放，本分支纳入 `44212e0` 的 `evidence/cn-pgo/` 与两个 CN 脚本原样快照；没有改写旧 raw、summary 或当时的 README 结论，当前限定以本文为准。CR reducer 将每份旧 evidence 与该固定 commit 的 Git blob 逐字节比较。

原 scratch 材料全部在，无需重建：

| 材料 | SHA256 |
| --- | --- |
| A | `8cbbcf1c1eba1c92a151b43ee23548db9b754964a364f9f8dce73ad6bfaf57fa` |
| profile-use B | `0de0526b7352b48bcbfb657c7d9393d1fa9956d46f9406ccfe275a5aa9e3f3f3` |
| merged profile | `aae1bb6b2b42bcd1bf76160aa192df0228998ae9805aa7e7d60ea38933993d0a` |

来自 `~/.hermes/cache/scratch/cn-pgo/` 的同一二进制、sidecar、profile 已在采集前验证，A/B 在采集后再次核对。原 Rust1.96 / LLVM22.1.2、显式 aarch64-apple-darwin、正式无插桩身份全部保留。本次零 Cargo build、零 Rust suite、零下载、零 profile merge/train、零新 target/worktree。

2026-10-16 以下时间均为 UTC+08:00：

- CN A/B gzip header 创建秒为 08:28:46，外层命令 wall 1.365486708s；A/A 创建秒为 08:28:48，wall 0.347978291s。header 只有秒精度，且 gzip 在 driver 开始后创建，不是每请求的开始时间，不能反推精确逐请求时间线。checkout 后的文件 mtime 不用于历史证据。
- CN 原 execution/manifest 没有同期 CPU、竞争进程或每请求 wall-clock 记录。用户报告另一个程序死循环吃 CPU，现有 CN 数据无法证实或排除。原 branch tape A0/B1/B2/A3 为 37.271/20.167/7.729/9.771 µs，确定存在耗时漂移，但原因不能归结给某个进程。
- 只读核对现有 CO CPU raw 的记录是 08:49:10–13，晚于 CN 约20分钟，不能填补 CN 时间窗；CO 的首次 snapshot 也不是有效区间 delta。本卡没有借用 CO 性能数值，更未将 CO 代码合入 PGO 两臂。
- CR A/B 08:58:41.832960–42.243904，A/A 08:58:42.243970–42.589731。完整进程 wall 含启动与 driver 开销，与下表请求计时不同。

## CPU 记录与停止规则

复用外盘 `target -> /Volumes/M.2/caches/minipy-native-target` 和 `minipy-perf-build.lock`。preflight 在同一锁内，原 driver 分别在 campaign 内持锁；锁只能协调本项目，不能隔离其他程序。本次没有停止、暂停或杀掉任何无关进程，无硬 CPU 独占、绑核或全局系统配置变更。

工程 guard 事先固定：top 1Hz，按 CPU 排前12进程；preflight 三次 snapshot，排除第一累计 snapshot，末两次均 idle≥90%，无无关进程≥50%单核。期间任一有效 snapshot idle<85% 或无关进程≥50%单核，停止自身 benchmark process group、保留部分 raw，不增加样本重救。这是保守工程门槛，不是统计噪声界或抢占检测器。

实际 preflight 有效 idle 97.30% / 96.19%；主要竞争 VM 为13.5% /14.3%单核。连续监控原文、进程表和处理时刻全部保留在 `collection.json.gz`：第一累计 snapshot 不计，随后三个有效 delta idle 为97.83%、94.76%、91.46%，均未触发停止。监控 delta 中主要竞争 VM 最高14.1%单核，另有 Finder、kernel_task、Python、网络系统进程。没有运行中的 Cargo/rustc 热负载。

重要的时间标签限定：collector 的 `begin_epoch/end_epoch` 是当前 top block 从收到 header 到收到下一 header 的输出包络；CPU 数值实际对应前一个 snapshot 到当前 snapshot。reducer 用前一 `begin_epoch` → 当前 `begin_epoch` 重建近似 CPU 区间，不直接采用 raw 的 `overlapping_valid_samples` 标签。header 接收也存在 top 扫描/管道延迟。A/B 由有效 delta index2、3覆盖，A/A 由index3覆盖；这只是约一秒系统聚合观测。两个 campaign 分别仅0.411s、0.346s，不能证明每个微秒请求都无抢占、频率/缓存漂移或监控开销。保留原 collector 与 hash，未因这个解释修正而重做计时；末尾可能不完整的 top block只保留文本、不当作有效delta。

## 同一 holdout、同一完整请求边界

双后端×branch/scalar_inputs/madd_loop/escaped_join/detach_wait，原 registered entry，一轮 ABBA，每进程12正式新源码请求，sample0原样排除。A/B 与 exact-A A/A 各40成功进程/480正式请求，合计80进程/960请求，精确 status/stdout/等待断言通过，零失败。没有选样、追加轮数、训练或改变顺序。

请求内仍含 prepare、execute、生产者构造/导入、code/output及等待控制器清理。source/expected 构造、fixture startup、进程启动、top与身份哈希在请求计时外，边界与旧 CN 一致。fixture startup中位数另列：A/B 69.334µs，A/A 72.146µs；不是进程启动时间，也不当作 PGO startup 比较。

下表是两份进程中位数的臂中位数之比，B/A低于1表示候选更快。与旧组分别计算，完整raw和每进程min/max/median均保留。不是精确因果百分比。

| Holdout | tape A→B µs | CR tape B/A | CR tape A/A | native A→B µs | CR native B/A | CR native A/A |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| branch | 9.479→8.125 | .85713 | .98564 | 69.396→53.281 | .76779 | .99237 |
| scalar_inputs | 3.969→3.761 | .94753 | 1.01657 | 52.115→42.469 | .81491 | 1.00321 |
| madd_loop | 16.000→14.010 | .87564 | 1.03502 | 123.323→89.344 | .72447 | 1.00144 |
| escaped_join | 14.386→12.240 | .85084 | .95527 | 84.865→63.875 | .75267 | 1.00390 |
| detach_wait | 80.010→75.239 | .94037 | 1.02156 | 159.083→134.250 | .84390 | .99987 |

native五项方向一致，且幅度大于本次独立A/A移动，与旧CN .76892/.80421/.72633/.71280/.84537的方向相符，值得保留PGO假设。A/A不是噪声上限，也没有真实Host源分布、跨机、吞吐/RSS、生产精确收益或无回归证明。

耗时漂移显著收敛但未消失：CR tape branch A0/B1/B2/A3为9.708/8.291/7.959/9.250µs；native branch69.312/52.812/53.750/69.479µs。CR tape scalar_inputs A0/A3为3.625/4.312µs，B1/B2为4.417/3.104µs；native escaped_join B1/B2为60.459/67.291µs。这些小样本波动保留，不拿当前较小漂移证明过去CPU污染，不用tape统一百分比概括所有形状。

## 取舍与仅一项后续建议

- Keep：固定训练profile的Rust-host PGO研究材料、native完整新源请求的正向信号。
- Downgrade：旧CN性能幅度作为未控环境历史观测，保留原raw与正确性/身份证据；旧组不再作为空闲对照或精确因果收益。没有依据撤销其全部正确性验证，也没有依据认定死循环确实污染了它。
- Defer：默认生产PGO仍关闭，main不合入该分支；profile维护成本和真实请求覆盖尚未成立，CR不声称PGO调节空间耗尽。
- 单一训练覆盖改进建议，未执行：在未来获准PGO实验中，用固定、去敏、按真实Host频率抽样的独立源码回放语料替代当前五个手写训练fixture，冻结现有holdout不参加训练，再用同一有界对照验证泛化。没有这类语料就继续defer，不以补齐所有测试monomorphization警告为目标，不搜索LTO/CPU/多轮参数。
- CP继续基于已审阅CO标量候选、未启用PGO的release比较SIMD；不要把本次PGO幅度与kernel收益相乘或混入同一基线。

## 回放与验证

实际执行：

    python3 tools/cr_pgo_recheck.py
    python3 tools/cr_pgo_reduce.py > evidence/cr-pgo/summary.json
    python3 -m unittest discover -s tools -p test_cr_pgo.py -v
    python3 -m unittest discover -s tools -p test_perf_workbench.py -v
    python3 -m py_compile tools/cr_pgo_recheck.py tools/cr_pgo_reduce.py tools/test_cr_pgo.py
    git diff --exit-code 907eb40 -- src tests Cargo.toml Cargo.lock
    git diff --check

四项CR解析/阈值/失败关闭/完整证据测试和九项BT契约测试全部通过。只读reducer复查两组身份与source hash、10 cells×双campaign完整性、原CN字节身份、真实CPU文本/近似区间及独立归约，不启动benchmark或Cargo。collector拒绝已存在output目录；禁止删除它再跑以提高分数。所有原raw、失败状态和取舍保留，未增加第三对采样。

原CN raw约1.8MB压缩payload只是原Git blob引用，未复制保留binary/profile进库；CR只新增小型raw/CPU/summary/脚本/文档。无新依赖、下载、CI、Linux/Wasm/Monty/Code Mode覆盖提升结论。完成后停止CR。
