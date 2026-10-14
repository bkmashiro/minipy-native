# AD：COW / JSON prefork 修正实测

已完成v2 300/300命令，11,769测量行+300初始化行，全部输出/Tool次数、三轮样本、first分段加性和PSS进程树基线通过；完整raw在本机逐值复算汇总一致。5项Go、8项Python、16项实机smoke通过。原pilot保存但不混池，不再补测。

同一Go producer逐请求生成Quote，两侧保留JSON→dict→SimpleNamespace和真实跨隔离传输。native为预热父进程每请求fork、compile/exec、退出并waitpid，不是pool。源码读取/适配与oracle在计时外，user compile在内；CPU包含Host、parent及已回收child，采样无RPC。生产runtime未改。

| workload | COW wall ms | prefork wall ms | COW/prefork |
|---|---:|---:|---:|
| scalar | 3.092 | 3.221 | 0.960× |
| weighted 10k | 139.313 | 31.437 | 4.432× |
| stateful 10k | 132.182 | 30.996 | 4.264× |
| weighted 100k | 1403.057 | 223.674 | 6.273× |
| stateful 100k | 1322.705 | 217.748 | 6.074× |

以上为三独立process rounds均值。scalar量级接近，大扫描差距仍在，未做分项因果归因。resident另列（scalar0.539ms），不能替代每请求隔离baseline。

mixed light、两次Tool各1ms：COW/prefork短任务p99为274.222/725.564ms，CPU/task为78.988/17.244ms，有限吞吐12.67/55.09req/s。COW短尾较低、CPU及吞吐较差，不能只挑一个维度定胜负。

32任务record_wait总PSS为672.795/391.733MiB，匹配增量276.771/285.572MiB。fork减同一批empty child；COW减实例创建前runner startup，基线生命周期要标清。现有1/8/32 counts可算KiB/task，无需另跑。JSON传输bytes不是对象逻辑owner或严格物理下界。

同运行首次scalar总wall为COW7.460s、prefork76.764ms，稳态3ms不是首次启动。

AE直接导入：`bench/results/ad-wsl/import-summary.json`；精确整臂替换清单：`replacement.json`。旧Pysolate curve/mixed/memory/first完整替换；JSON prefork为主要baseline，resident可选；其他引擎保留原版本身份。配对组为`ad-json-object`，不暗中并成旧typed/namedtuple共同表示。网站未由本卡修改/部署。

方法、范围、原始数据与复现见[完整报告](corrected-prefork-benchmark.md)。
