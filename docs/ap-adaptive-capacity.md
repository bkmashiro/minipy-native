# AP 自适应容量与 p99 加密

## 实验边界

本轮只扩展 AF 的 CPU 与 mixed 两个 source-once 工作负载、五条执行路径。
冻结 AF 提交为 `5e047679d19e002f064de6dc52516b5d6220eea1`，runtime 源码仍为
AF 的 `969e679` 身份。使用 WSL 原有 `af-capacity/interpreter_ab` 与
`af-capacity/worker`，没有重建 Guest、修改生产 runtime 或混入 AJ/AL/AN。

原 AF `identity.json` 的所有文件与 canonical raw 的 SHA256 在前后各验证一次，
Go adapter 实际读取的 `source/fixtures` 也逐文件对照冻结快照。
运行时环境另外记录 kernel、Python、CPU 型号与 CPU2/4 SMT sibling。
这能确认文件/配置身份，不能控制 Windows 活动或物理主机外部负载。

保留 AF 的开放到达、source 每请求准备、真实输入生成和执行、64 waiting /
16 inflight、有界过载提前停止及 30s drain。service 整树 CPU2，driver CPU4，
两把原有共享锁 `compute.lock` / `wsl-heavy.lock` 在整个采集期间持有。
所有 latency 都从计划到达计起，不丢掉 driver lateness、排队或编译成本。
服务初始化仍在每独立进程的 ready 之前，不在请求计时内。

## 选点与预算（工程选择）

使用同一规则处理每条曲线，允许不同 rate 网格：

1. 每曲线在原 AF stable lower rate 跑一个 15s 锚点。每完成请求服务 CPU
   与原三轮中位数相差超过 25% 时复测。两次均超界，才判定该曲线漂移，
   该曲线旧点不进入有效曲线，按原 rate 补测种子。p99 超原三轮最大值两倍
   也触发锚点复测，单独的尾部波动不冒充 CPU 漂移。
2. 先补原 stable lower 的 25%、50%、75% 三个位置，展示平台与渐升过程。
   稳定/过载区间再取中点，目标相对宽度 ≤5%。若无稳定或过载端点，
   分别将最低 rate 减半或最高 rate 乘 1.5，直到得到 bracket 或触及预算。
3. 至少两个过载区域 rate。相邻 p99 相差 ≥1.5 倍且 rate 间距大于原
   stable lower 的 5% 时，在中点加测。不是只二分一个容量阈值。
4. 曲线轮转，每轮每条曲线最多一个新空间点。前七轮后优先独立进程确认
   拐点端点，再继续曲线加密。分类不一致或同 rate p99 范围超过两倍时，
   独立复测，保留噪声和所有原始观测，不择优丢弃。
5. 总采集预算 4200s、最多 600 万新增 offered（初步成本估计为 300 万，
   执行前为防高吞吐臂先耗尽样本额度调整到 600 万）。每次启动前为预计
   arrival 时间另留 35s launch/drain 余量；没有点数三点硬上限。
   空间目标、三轮确认和尾样本支持分别判断，耗尽预算明确标记未收敛。

探索轮为 `max(11, min(30, 3+100/rate))` 秒，其中 warmup 3s。
确认轮为 `max(15, 3+1000/rate)` 秒，每轮目标至少 1000 个 measurement
请求，三个独立进程。过载时仍保留 AF 提前停止规则，不为凑尾部样本去掉
资源界限。低速旧 AF 点与轻量探索点只能作探索观察。

1000 次完成约只在最上方 1% 留下十个样本，是本轮工程筛选下限，
不是 p99 精密置信保证。混合任务有确定的 40 请求周期，不假定 IID。
报告独立轮百分位的 mean/min/max，不混池计算伪造独立样本数。
过载有限 p99 只对完成请求成立，必须同时展示拒绝、失败、超时、未发出
与提前停止；`p99_precision` 不足的点不能画成精确尾部确认。

## 存储与复现

原始请求不复制到 Mac，也不复制第二份 AF raw。新增 `ap-*.jsonl.gz`
仍位于 `/home/na/benchmarks/minipy-s/af-capacity/`，与原 AF raw 名称分离。
`ap-capacity/state.json` 只存选点与进度，summary/index/身份文件放
`/home/na/benchmarks/minipy-s/ap-capacity/`。仅小型派生数据进入 Git。

命令（在 WSL，脚本与原 AF driver/reducer 同目录）：

    flock -n /home/na/benchmarks/minipy-s/compute.lock \
      flock -n /home/na/benchmarks/minipy-s/wsl-heavy.lock \
      taskset -c 4 python3 /home/na/benchmarks/minipy-s/af-capacity/ap_capacity.py \
      /home/na/benchmarks/minipy-s campaign

    python3 /home/na/benchmarks/minipy-s/af-capacity/ap_capacity.py \
      /home/na/benchmarks/minipy-s reduce

本地算术/选点回归：

    python3 -m unittest discover -s tools -p 'test_a*_capacity.py' -v

## 网页交接约束

继续使用现有 capacity UI、默认高亮 IR/native 和现有 baseline 列表，
不新增顶层配对组、不更名、不部署旧站。只连实测点，用直线而非平滑曲线。
原 importer 写死 AF cohort / 30 points / 90 runs / 每点三轮，必须按 AP
schema 显式适配，不能只替换 JSON。AP 独立轮数、窗口长度与探索/确认阶段
不再恒定。逐点保留 `round_keys`、`phases`、`measurement_completed_per_round`、
`full_sample_rounds`、`p99_precision`、counts、early-stop 与 cancelled-unoffered。
不要将空间 bracket 已窄与三轮样本已确认混为同一个“精确容量”。
绘图使用 `plot_latency_ms` / `plot_round_keys`：若存在至少三轮各≥1000次
measurement 完成，只聚合这些足样本轮，短旧点/探索轮仍保留于完整
`latency_ms` 与损失账本，不借用确认标签。否则该点明确显示 exploratory/censored。

网页聚合不携带 argv、主机路径或 full trace，展示表示差异短标签。
禁止将 typed/Monty object 与 JSON dict/SimpleNamespace 当成严格同表示倍率。
既有 AD/S/W resident / Rust / ASM 证据保留，不拼入本 source-once 曲线。

## 采集中的一次控制器修正

前77份新增 raw 完成后，发现某已测中点只表现尾部不稳定，既不是稳定端点
也未满足明确过载分类。原选择器仍算出同一中点并读取缓存，没有新增信息。
以 SIGINT 触发 AF `finally` 清理当前 worker，保存原 controller 与中断说明，
改为细分未测子区间，含糊状态另作独立复测。新增回归保护“不重复选择已测
含糊中点”。worker、AF driver、定时边界、旧 raw 和已完成新 raw 都不变。

中断时 `ap-explore-monty-mixed-25-0` 尚未写出 raw（AF 在完成后一次持久化），
因此该次中断的请求数未知，明确 excluded，不计0也不混入完整raw账本。
同 rate 后续完成的新运行具有自己的完整数据。采集预算保守扣除1180s后续跑，
不重置总预算。`controller-v1.py`、`controller-interruption.json` 及前后身份
记录保留版本演化。汇总器也明确区分新增与含复用点的有效全量账本，
并将 variable duration 写入方法契约，避免继承 AF 固定15s文字。

## 实测结果与未收敛项

采集预算用时4159.796s（含中断前保守扣除1180s），新增158个完整独立运行：
10 anchor、86 explore、62 confirm。新增86个不同rate位置，最终含原90轮种子
共116点/248轮，每曲线10–14点。没有将原三点硬上限换成另一个点数上限。

完整新增raw中2338861 completed、1617 rejected，合计2340478 offered；
failed/timeout均为0，66409 cancelled-unoffered不计入offered。
含原AF有效总账为3164269 completed、46272 rejected、3210541 offered，
528931 cancelled-unoffered。上述总账仅涵盖完整持久化raw，不包含前述请求数
未知的中断attempt。公开投影也以 `excluded_attempts` 明示该项，不能补0。

158份新增compressed raw合计118062432 bytes，仍只存WSL canonical目录。
原90份raw与41个冻结身份文件在前后均hash匹配。每份新raw逐请求重新验证
oracle、id、status、账本、producer/Tool总数及sidecar百分位，158份全部匹配。
公开投影再验证全部248轮与116点的账本、完整/plot子集百分位和路径剔除。

所有曲线在本轮预算内均标为未收敛：一部分仍需曲线加密，一部分有独立轮
分类分歧/非单调观测，不能给出确定容量。下表是有限窗口的观察区间，不是
置信区间，更不是部署路由阈值。confirmed lower要求该rate所有已有轮通过
稳定筛选，且至少三轮各≥1000 measurement完成（长尾仍有显著轮间范围）。

| 工作负载 | 路径 | 最大一致稳定观察 req/s | 明确过载观察 req/s | confirmed lower req/s | 点数 |
|---|---|---:|---:|---:|---:|
| CPU | IR | 4100 | 5000 | 3800 | 10 |
| CPU | native | 1650 | 1750 | 1650 | 12 |
| CPU | Monty | 52.5 | 65 | 未确认 | 10 |
| CPU | fork JSON | 32.5 | 35 | 32.5 | 12 |
| CPU | COW | 6.5625 | 6.9375 | 6.5625 | 14 |
| mixed | IR | 5781.25 | 6250 | 5781.25 | 10 |
| mixed | native | 2075 | 2250 | 2075 | 10 |
| mixed | Monty | 55 | 62.5 | 未确认 | 14 |
| mixed | fork JSON | 55 | 57.5 | 55 | 11 |
| mixed | COW | 12.625 | 13 | 12.625 | 13 |

加密比原三点更清楚地揭示了尾部，而没有消除窗口/轮间噪声。例如：

- native CPU 1450 req/s的足样本p99均值3.733ms（独立轮范围3.406–4.285），
  1650为14.427ms（12.480–17.627）；1700多轮分类不一致，条件p99约31–47ms。
- COW CPU 6.5625的四个1000-completion确认轮p99为141–1617ms，轮间跨度
  很大，不能用均值864ms掩盖它；6.75的三个足样本确认轮为1500–1539ms，
  但同rate短探索曾被分类为过载，因此该点整体仍标inconsistent。
- COW mixed 12.625三个足样本确认轮p99为15347–16508ms，显著高于同rate
  100-request短探索的约6003ms。只增加横坐标密度而不延长慢路径会漏掉此尾部。
- Monty CPU 53.75/55及mixed56.875/58.75的独立分类分歧保留；两条Monty
  曲线本轮没有得到三轮足样本且一致稳定的下界。不是0吞吐，也不是不支持。
- IR CPU在3950曾出现过载而更高rate有稳定观察，Monty mixed也有类似非单调
  点。公开投影列出 `nonmonotone_overload_rates` 和 `inconsistent_rates`，
  这些曲线不被提升为单调容量证明。

原始实测点可用于现有网页曲线，但必须展示marker、独立轮范围、样本/阶段和
损失数，禁止平滑拟合或隐藏低样本/不一致点。`public-summary.json` 为可公开
的小型投影，`import-summary.json`保留完整派生trace供私有复核。
本轮没有编辑网页仓库或部署。复现公开投影：

    python3 tools/ap_public.py bench/results/ap-capacity/import-summary.json \
      bench/results/ap-capacity/public-summary.json

停止原因是预先声明的时间预算，未达到所有曲线收敛目标，不自动追加下一轮。
