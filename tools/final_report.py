"""L bilingual-ready evidence report. All numeric tables derive from summary.
Run after final_summary.py and final_workload.py. No historical result splicing.
"""
import json
from pathlib import Path
import statistics as st
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'bench/results/l-final'
S=json.loads((OUT/'summary.json').read_text())
W=json.loads((OUT/'workload-accounting.json').read_text())
LABEL={'native':'MiniPy codegen','interpreter':'MiniPy typed IR','monty':'普通 Monty','reference':'固定 Rust 程序参考',
       'cpython-spawn':'CPython 每请求 spawn','cpython-fork':'CPython 预热后 fork','cpython-resident':'CPython resident / fresh source',
       'cpython-prepared':'CPython resident / prepared source','pyodide-resident':'Pyodide resident / fresh source','pyodide-prepared':'Pyodide resident / prepared source'}


def table(headers,rows):
    return '| '+' | '.join(headers)+' |\n| '+' | '.join(['---']*len(headers))+' |\n'+''.join('| '+' | '.join(map(str,r))+' |\n' for r in rows)


def value(r,key):
    if r[key] is None:return '未测'
    lo,hi=r['round_ranges'][key]
    return f'{r[key]:.2f} [{lo:.2f}, {hi:.2f}]'


def shape(pool,engine,case,n,mode):
    return next(r for r in pool if (r['engine'],r['case'],r['rows'],r['mode'])==(engine,case,n,mode))


def main():
    assert {'common','assembly','scalar_recheck'}<=S.keys() and not S['coverage']['missing']
    assert S['assembly']['full_cohort']=='assembly-full.jsonl.gz'
    primary=S['primary']['shapes']
    text='''# 端到端成本：源码准备/编译 + 执行

本轮冻结生产源码 `8a293ec54c6e7aed72e23a5c1557a3bdc3b44f60`。全部速度/内存 arm 在同一台实际 Linux ARM64 VM 顺序运行；本轮没有产品优化、自动 backend 选择、源码 cache 或发布 CI。

结论：源码冷的小程序不适合承担 native compilation；明确复用准备代码或扫描较大 typed buffer 时，codegen 更有价值。普通 Python resident 标量很便宜，typed data 路径的价值主要体现在避免对象物化、数据移动和大扫描开销。预热进程、预编译固定程序与未知源码准备属于不同生命周期，不能合成一个“最快语言”排名。

首次四路矩阵、后增 CPython/Pyodide、共同源码和手写汇编各有独立 raw/identity。本轮不从 J/K 或其他机器拼数字，也不据此计算历史优化幅度。

## 图与证据

![冻结 MiniPy 的成本与等待态观测](figures/benchmark-overview.svg)

图中 source-cold 包含源码解析、类型准备/编译、执行、输入构造和回收，但 worker process/Tokio runtime 已驻留。Prepared 是显式 code reuse，每请求仍有新状态和 Provider/input，不是免费初始化。固定 Rust 参考只出现在 prepared panel，没有未知源码准备。

![普通 Python 的进程生命周期与内存](figures/benchmark-python-lifecycles.svg)

第二张图来自原始适配语料：Monty/CPython/Pyodide 扫描 tuple rows，MiniPy 扫描 Quote/v1。下面的共同源码表是新增 cohort，不把其 timing 与原始 tuple cohort 的内存混称一次运行。

机器入口：[`summary.json`](../bench/results/l-final/summary.json)、[`plan.json`](../bench/results/l-final/plan.json)、[`addon-plan.json`](../bench/results/l-final/addon-plan.json)、[`workload-accounting.json`](../bench/results/l-final/workload-accounting.json)。SVG 有同名 PNG 预览。摘要字段保留 unit、mode、round ranges、缺失值和 overload，适合 demo 消费；图中数值只来自 summary。

## 环境与统计边界

- Apple M4 / Mac16,10、宿主 10 logical CPUs、16 GiB RAM，macOS 26.4.1；guest Alpine Linux 3.24 ARM64、Linux 6.18.52、2 vCPU / 2048 MiB。预/后检查没有 swap 使用；没有单独采样宿主温度、频率或物理核映射。
- Rust 1.96.0、Zig 0.15.2、`aarch64-unknown-linux-musl`，Monty `56949a4c45f51cedc7a96450782f58a8017e114d`。CPython 3.14.7；Pyodide 314.0.7 内部 CPython 3.14.2；task-local Node 24.18.1。Node/依赖只解压到 guest 本任务目录，没有全局安装。
- 全轮未 pin，也未中途改变 affinity。VM 内没有并行跑速度/内存 campaign；计时时暂停重编译。宿主调度/其他桌面负载仍可能干扰。2 vCPU 是受控相对对照，不能当裸机绝对值；guest `taskset` 也不等于物理核独占。
- 三轮按 arm 轮转，奇数轮反向。标注值是请求均值、batch 总 CPU/成功数或 pooled nearest-rank 分位数；方括号/图中细线是三轮统计量的观测 min–max，不是 CI。独立 `n=3`，请求数不是独立 n。
- 原始 shape 101 个请求保留 sample≥21；mixed 9 个窗口保留≥2。共同源码 primary shape 41 个保留≥11，mixed 5 个保留≥1；Python shape 21 个保留≥1。汇编 101 个保留≥21。所有 warmup raw 都保留。
- 速度 binary 不带 allocation diagnostics/trace。内存用独立新 process 和 counting/tracemalloc diagnostics。Rust requested Layout bytes、Python tracemalloc 与整个 runtime allocation 不等价，缺测保持 null；跨 runtime 主图用整个参与 process/process-tree PSS。

## 原始四路：冷总成本与显式复用

下表单位均为 µs，值为总体均值 [三轮均值最小值, 最大值]。`prepare`/`execute` 是直接计时的 wall phase，不是 CPU stage，也不相加 phase medians 构造未测总时间。

'''
    rows=[]
    for case,n in [('scalar',0),('weighted',10000),('stateful',100000)]:
        for e in ['native','interpreter','monty']:
            r=shape(primary,e,case,n,'cold');warm=shape(primary,e,case,n,'warm')
            rows.append([f'{case}:{n:,}',LABEL[e],value(r,'wall_mean_us'),value(r,'cpu_mean_us'),value(r,'prepare_mean_us'),value(r,'execute_mean_us'),value(warm,'wall_mean_us')])
    text+=table(['程序/rows','执行路径','cold wall','cold CPU','prepare wall','execute wall','prepared wall'],rows)
    text+='''
Native 在小标量中承担明显 compilation 成本。主矩阵 light/scan 的平均 cold CPU，typed IR 通常较少；native/IR 的小幅平均 CPU 差距有 round variability，不能只取一个比例。Prepared native 在含大扫描的混合中明显减少 CPU，而 source-cold compilation 会阻塞同一 dispatcher 上其他任务的计划到达。

### finite-window 混合、容量与 overload

Light 权重 `(scalar0, weighted100, stateful100, weighted10k, stateful100k)=(24,8,4,2,2)`；scan 为 `(8,16,8,4,4)`。每窗口 40 个计划到达，间隔 500 µs，名义窗口 100 ms。latency 从计划到达开始，含到达/编译/admission 排队；完成后等到窗口结束，超时窗口先 drain 再开始下一窗口。没有持续推进固定到达时钟，因此不能当持续 400 requests/s 生产 SLO。

`2×1ms` 是两次实际 Tokio/asyncio timer await 的请求参数，不保证实际总 wait 精确 2ms，不是网络/数据库实测。短任务是 rows≤100 的固定人工组合；scalar 不调用 Tool。以下为 source-cold，CPU/请求、平均 latency、短 p50/p95/p99 均为 µs。

'''
    rows=[]
    for r in S['primary']['mixed']:
        if r['inflight']==40 and r['mode']=='mixed-cold' and r['engine']!='reference':
            rows.append([r['mix'],r['delay_us'],LABEL[r['engine']],value(r,'cpu_us_per_task'),f"{r['mean_latency_us']:.2f}",f"{r['short_p50_us']:.2f}",f"{r['short_p95_us']:.2f}",f"{r['short_p99_us']:.2f}",f"{r['overloaded_windows']}/{r['windows']}",f"{r['finite_window_requests_per_s']:.1f}"])
    text+=table(['mix','Tool delay µs','路径','CPU/成功数','mean latency','short p50','short p95','short p99','overload windows','finite requests/s'],rows)
    text+='\n容量 probe 固定 scan/source-cold/两次 1ms await，改变 admitted inflight=1/8/32，源/到达计划不改。排队在 latency 中，所有请求完成；这里不是最大可承载并发搜索。\n\n'
    rows=[]
    for r in S['primary']['mixed']:
        if r['inflight']!=40:
            rows.append([LABEL[r['engine']],r['inflight'],f"{r['mean_latency_us']:.2f}",f"{r['short_p99_us']:.2f}",f"{r['overloaded_windows']}/{r['windows']}",f"{r['finite_window_requests_per_s']:.1f}"])
    text+=table(['路径','inflight','mean latency µs','short p99 µs','overload windows','finite requests/s'],rows)
    text+='''
### 小 warm 的一次有据补测

初次 warm 标量的 codegen round 均值在 3.11–12.40µs，明显不稳定。按预登记 probe，用原始冻结 speed binary 做 3 轮 × 1001 请求、保留≥201；不替换主矩阵或和它合并。较长 sample run 的工作条件不同，不能用它计算主矩阵优化倍数。

'''
    text+=table(['路径','单独复测 warm wall µs'],[[LABEL[r['engine']],value(r,'wall_mean_us')] for r in S['scalar_recheck']])
    text+='\n补测支持“小程序 steady-state 本身很便宜”；native/IR 的细微标量排序受运行长度、scheduler、timer/CPU accounting overhead 影响，不以此选一个通用 backend。\n'
    text+='''
## 实际 work，不只看 row count

这些逻辑计数由相同生成输入离线逐行复算，没有向速度 hot path 加计数器。它们是 source-level 分支/算术工作量，不是硬件分支或 CPU utilization 测量。`stateful:100k` 是大扫描、主要执行 predicates，不称其为高算术强度 workload；输入构造、对象物化和 predicate 仍有成本。

'''
    rows=[]
    for r in W['rows']:
        if r['rows'] in [10000,100000]:
            rows.append([r['case'],r['rows'],r['quantity_positive'],r['price_filter_hits'],r['selected_rows'],f"{100*r['selected_fraction']:.3f}%",r['accumulator_multiplications'],r['accumulator_additions'],r['state_updates']])
    text+=table(['程序','rows','quantity>0','再通过 price','最终选中','选中比例','accumulator mul','accumulator add','previous 更新'],rows)
    text+='''
## 新增真正共同源码主体

可展示程序是 [`common_scalar.py`](../fixtures/common_scalar.py)、[`common_weighted.py`](../fixtures/common_weighted.py)、[`common_stateful.py`](../fixtures/common_stateful.py)。MiniPy 与 Monty 直接读取逐字相同 UTF-8 body；两者都支持这里的 top-level await。CPython/Pyodide 的整个 source 多一个 async function signature 和四格缩进，显式记录于 `common-cpython-identity.json.wrappers`；去缩进逐字还原 body 的断言和测试通过。未改 Tool 名、属性访问、算法、谓词顺序或算术表达式。

适配在 Host：MiniPy 是 immutable Quote/v1；Monty 是 `tools` module + eager HostClass Quote fields；CPython/Pyodide 是 signed-64 packed array 转 Quote namedtuple list。每请求 Provider、数据生成/转换、输出与回收在计时内；函数 wrapper 构造/compile 在 fresh-source timing 内，prepared mode 明确复用其 code。来源配置/benchmark 选择和 IPC outside-boundary 明确不同：resident 单请求没有 fake child process/IPC，spawn/fork 含真实 IPC/teardown。

初次 Monty namedtuple adapter 的 Tools 属性访问在固定 revision 上失败，未进入正式 common raw；改为其已存在的 eager HostClass/module 接口。失败 identity 保留。

以下为共同主体的实际 lifecycle wall µs，不把三种输入 ABI 当相同内部操作，也不把其结果解释为纯编译器/纯整数算术排名。共同主体没有新增 waiting-memory cohort，原始 tuple cohort 的 PSS 不迁移到这张表。

'''
    pool=[r for v in S['common'].values() for r in v['shapes']]
    rows=[]
    for e in ['native','interpreter','monty','cpython-resident','cpython-prepared','pyodide-resident','pyodide-prepared']:
        mode='warm' if e.endswith('prepared') else 'cold'
        rows.append([LABEL[e],mode,*[value(shape(pool,e,c,n,mode),'wall_mean_us') for c,n in [('scalar',0),('weighted',10000),('stateful',100000)]]])
    text+=table(['路径','source lifecycle','scalar0 wall','weighted10k wall','stateful100k wall'],rows)
    text+='\nCPython/Pyodide 的转换分项直接覆盖 packed input 生成及 Quote object construction，均为 wall µs：\n\n'
    rows=[]
    for e in ['cpython-resident','pyodide-resident']:
        for c,n in [('weighted',10000),('stateful',100000)]:
            r=shape(pool,e,c,n,'cold');rows.append([LABEL[e],f'{c}:{n}',value(r,'conversion_mean_us'),value(r,'wall_mean_us')])
    text+=table(['路径','程序','input generation + conversion wall','whole request wall'],rows)
    text+='\n共同主体 scan mix 的 ready 路径（每个路径使用表中标明的 source lifecycle），CPU/请求、平均 planned-arrival latency 与 short p99 为 µs：\n\n'
    rows=[]
    for v in S['common'].values():
        for r in v['mixed']:
            if r['mix']=='scan' and r['delay_us']==0:
                rows.append([LABEL[r['engine']],r['mode'],value(r,'cpu_us_per_task'),f"{r['mean_latency_us']:.2f}",f"{r['short_p99_us']:.2f}",f"{r['overloaded_windows']}/{r['windows']}"])
    text+=table(['路径','source lifecycle','CPU/成功数','mean latency','short p99','overload windows'],rows)
    text+='''
### 整数、authority、取消的边界

相同主体和有效输入只证明这些正常结果/调用顺序相同。生成数据 price 0–999、quantity -2–6、limit700、batch17，所有选中 arithmetic 及 receipt 都在 i64 范围。MiniPy/汇编保留 signed-64 中间溢出、row/byte/Tool/print 预算和 chunk/cancel 行为；普通 Monty/Python 有任意精度整数，扫描是不同的 blocking turn，没有同一套 fuel/capability 机制。Python 的异步操作取消也不是 MiniPy 的 detached operation lifetime。CPython spawn/fork 的进程隔离不是相同语言 authority；Pyodide Node resident 不是浏览器隔离测试。本轮不评判任意恶意/错误程序的 sandbox 强度。

## 手写 AArch64 汇编性能参考

源为 [`weighted_aarch64.S`](../bench/src/weighted_aarch64.S)，受控 Host 是 [`assembly_reference.rs`](../bench/src/assembly_reference.rs)，可执行 harness 是 [`assembly_ab.rs`](../bench/src/bin/assembly_ab.rs)。实际手写 AAPCS64 指令：`mul` + `smulh` 检查 signed product，`adds` + V flag 检查 sum，保留 quantity/price/previous 的短路顺序。入口只接有效 immutable slice 和独立 i64 state pointers，没有任意用户机器码注入。

V1 的不变 mode 分支位于 row loop 内，108B；仅做一次外提，最终 checked kernel 的 code bytes 是 '''+str(S['assembly']['kernel_code_bytes'][0])+'''B，numeric sum/previous state 16B。这些不是 Task future/服务总内存。没有 unchecked 或 SIMD 假设版本。代码更短/手写不保证最快，weighted 的最终 kernel 与同轮优化 Rust 大致相当；stateful 仍较慢。V1 raw/source 与 V2 结果都保留，非配对的 V1→V2 不计算因果加速比例。

Kernel-only：input 创建/验证在计时外，计时内含 checked 256-row kernel calls + numeric state 初始化，无 Provider、stdout、async waits、未知源码准备。Full：实际相同 RequestProvider/Quote/v1，数据构造在每请求 timing 内，row/byte/Tool/print bounds、chunk yield、detached load/record/cancel、输出和回收均保留。Host 因生产 helper 为 crate-private，benchmark 中复制相同 cancellation/authority 机制；单行输出使用 format!，其额外 allocation 已计时。它是固定程序执行参考，不是通用编译 baseline 或已证明物理最优。

发现初次 full harness 还在 timing 外保留了无用预验证 fixture（实际 Provider 已在 timing 内生成另一个 input）。已去掉；最终 full table 使用 `assembly-full.jsonl.gz` 的新 3 轮配对数据，原 full raw 仅作历史诊断。Kernel-only 使用独立有效的 `assembly-v2.jsonl.gz`；表格不把两种 boundary 合成一个 e2e time。

单位 µs，均值 [三轮均值范围]，100,000 rows：

'''
    rows=[]
    for r in S['assembly']['shapes']:
        if r['rows']==100000:
            rows.append([r['case'],r['engine'],r['mode'],r['delay_us'],value(r,'wall_mean_us'),value(r,'cpu_mean_us')])
    text+=table(['程序','固定程序 arm','boundary','Tool delay µs','wall','CPU'],rows)
    text+='''
## Waiting memory 与 density（独立问题）

原始四路测 1/8/32 tasks、10k rows/task、unique/shared preparation；shared 意味 prepare-once + backend 自己的 duplicate 行为，Monty clone 不承诺全部 object memory 物理共享。每个 cell 是独立新 process，重复三次。Gate 观察 load wait、record wait、reclaimed；requested allocator bytes 与整个 process RSS/PSS 都在 raw，以下只列 8 tasks / unique 的 PSS（MiB）。Spawn/fork 包含 controller + 全部 living workers；PSS 求和分摊共享页，RSS 不作为物理总量相加。Node/Pyodide Provider 与 dispatcher 在同一参与进程。

这些是等待态 footprint，未搜索生产容量上限，也不据之直接推算每 GiB 最大用户数。Node/V8/WASM、scheduler、代码、Provider、data、输出/诊断状态均在 participating-process PSS 范围内；WASM tracing bytes 不是整个 V8 footprint。

'''
    rows=[]
    for e in ['native','interpreter','reference','monty','cpython-resident','cpython-spawn','cpython-fork','pyodide-resident']:
        pool=S['memory'] if e in ['native','interpreter','reference','monty'] else S['secondary_memory']
        phases=[]
        for p in ['startup','load_wait','record_wait','reclaimed']:
            r=next(r for r in pool if (r['engine'],r['tasks'],r['code'],r['phase'])==(e,8,'unique',p))
            phases.append(f"{r['pss_bytes']['mean']/2**20:.2f}")
        rows.append([LABEL[e],*phases])
    text+=table(['路径 / original adapted cohort','startup PSS','load wait PSS','record wait PSS','reclaimed PSS'],rows)
    text+='\n固定汇编 Host 的独立 memory process footprint（10k rows/task，static shared code；不可把其 minimal binary base 当同一通用 compiler service）：\n\n'
    rows=[]
    for r in S['assembly_memory']:
        if r['tasks']==8:rows.append([r['phase'],f"{r['pss_bytes']['mean']/2**20:.3f}",f"{r['requested_bytes']['mean']:.0f}"])
    text+=table(['phase / 8 tasks','whole-process PSS MiB','Rust whole-process requested bytes'],rows)
    text+='''
Native/IR 在已证明最后使用处释放 typed payload，后续 print 仍可要求保留 code/frame；普通 tuple/object corpus 的局部引用有不同 lifetime。Monty list-cleared 只作显式 fixture-specific 强参考，其 PSS 与 allocator reclaim 不必同一时间下降。Owner release、取消 suspended receiver、late provider completion 和 effect errors 另用真实 gate 生命周期测试验证，不用 frame bytes 替代服务内存。

## 新 process 初始化单列

下面是 in-VM controller 直接测完整新 process 生命周期，不含 SSH 往返。CPython 行包含新 interpreter/imports、source-cold scalar 请求、真实 IPC 和 reap；Node/Pyodide 行包含 Node/imports、WASM/stdlib/harness 初始化和退出，内部 init timer 也在该次 response 中。使用 current common-capable harness；不把这些值事后摊入旧 resident observations。单位 ms，三次独立新进程的均值 [观察范围]。

'''
    rows=[]
    for e in ['cpython','pyodide']:
        rs=[r for r in S['initialization'] if r['kind']=='process-initialization' and r['engine']==e]
        assert len(rs)==3
        vals=[r['wall_ns']/1e6 for r in rs];cpus=[r['cpu_ns']/1e6 for r in rs]
        rows.append([e,f'{st.mean(vals):.2f} [{min(vals):.2f}, {max(vals):.2f}]',f'{st.mean(cpus):.2f} [{min(cpus):.2f}, {max(cpus):.2f}]'])
    text+=table(['完整 process boundary','wall ms','total controller+child CPU ms'],rows)
    text+='''
## 覆盖、验证与限制

主四路 60,696 请求全部成功，108/108 speed cells、120/120 memory cells。新增 CPython 10,860、Pyodide 5,430 请求；共同主体 25,350 请求、84/84 cells；最终 fixed reference 40/40 cells（12,120 observations，kernel/full 来自明确不同 cohort）。一次 warm 长复测 12,012 行。历史汇编 V1/V2/full 修正前数据都保留，不混入 final full 表。正式数据无 timeout/failed request；部署 fixture 缺失与 namedtuple adapter smoke 失败有独立 identity，不能把 operational attempts 宣称零失败。

已实际运行：Linux AArch64 汇编 differential MIN/MAX/overflow/filter-order/chunk tests 与 whole Host permission/budget/cancel/late-completion tests；完整 assembly test binary 4/4。冻结 native/IR/reference lifecycle 在 macOS 与实际 Linux 均通过。Python reduction/common-wrapper 五个针对性测试通过。summary 可从 raw 复算；独立复算与最终聚焦 evidence/figure review 的记录随本轮交付。真实 PNG 已打开检查文字、图例、裁切和范围编码。

未测：浏览器 Pyodide、长期持续 open-loop SLO、NUMA/裸机 affinity、多核 capacity search、任意恶意 Python sandbox、安全等级等价、byte/string 输入微增量、common-body waiting-memory。未来裸机对照应固定同一 NUMA 物理核集合、避开 SMT siblings、记录 CPU/温度/频率和无 swap 压力，所有 paired arms 使用同一 affinity，不把本 VM 与未来机器混合排序。

本轮剩余热点主要是 cold native compilation、typed IR 的逐行 predicate/expression execution，以及普通对象 runtime 的数据构造/桥接和 blocking scans。规模本身不是算术强度指标。没有自动 cache/backend heuristics 提案，也没有从当前单机 synthetic mix 推导真实部署占比。

## 复现（不需要 CI、Docker 或全局安装）

分析/出图：

```sh
python3 tools/final_workload.py
python3 tools/final_summary.py
python3 -m unittest discover -s tools -p test_final_benchmark.py -v
uv run --with matplotlib python tools/final_plot.py
python3 tools/final_report.py
```

原始四路的 measured harness 已保存为 `bench/results/l-final/sources/interpreter_ab-primary.rs`，其 SHA 与原 identity 精确核对；`sources/final_python.py` / `final_pyodide.mjs` 是原 secondary cohort 的精确 source snapshot。新 common/最终汇编使用当前 tools/bench sources。不同历史 cohort 的 harness 不被当前 adapter silently 覆盖。

在干净的 frozen-source reproduction checkout 将 primary snapshot 放回 `bench/src/bin/interpreter_ab.rs`（不要覆盖在用的主 checkout），沿 `identity.json` 的 source/toolchain 和以下命令构建。新 campaign 应使用新 result 目录，runner 默认拒绝覆盖 raw：

```sh
cargo +1.96.0 zigbuild --release --locked --manifest-path bench/Cargo.toml --target aarch64-unknown-linux-musl --bin interpreter_ab
# 保存 speed；再单独构建 memory：
cargo +1.96.0 zigbuild --release --locked --manifest-path bench/Cargo.toml --target aarch64-unknown-linux-musl --bin interpreter_ab --features allocation-diagnostics
python3 tools/final_campaign.py HOST KEY KNOWN SPEED MEMORY
# 当前共同源码：同样 build interpreter_ab speed binary，开启 MINIPY_COMMON=1。
# 完整原 secondary 用 sources/ 下两个 sibling snapshots，Node package 对应 314.0.7。
python3 tools/final_secondary.py HOST KEY KNOWN
# 当前 common + 原始汇编追加的命令逐项保存在 addon-identity.json。
python3 tools/final_addon.py HOST KEY KNOWN COMMON_SPEED ASM_SPEED ASM_MEMORY ASM_TEST
# 最终 checked kernel/Host（先保存 speed；memory 独立 --features allocation-diagnostics）：
cargo +1.96.0 zigbuild --release --locked --manifest-path bench/Cargo.toml --target aarch64-unknown-linux-musl --bin assembly_ab
cargo +1.96.0 zigbuild --release --locked --manifest-path bench/Cargo.toml --target aarch64-unknown-linux-musl --bin assembly_ab --tests
python3 tools/final_assembly_refinement.py HOST KEY KNOWN SPEED MEMORY TEST full-only
# 单个可展示的 full reference / kernel（预编译固定程序）：
# 在 Linux ARM64：assembly_ab assembly full weighted 100000 101 1000
#             assembly_ab assembly kernel weighted 100000 101 0
```

Node/Pyodide task-local建立的逐项命令在 `secondary-identity.json`，APK HTTP download failure 后改用 HTTPS，未改系统 package/config。guest package tarballs、编译树与大可再生 artifacts 不提交。生产库、dependency lock 与 threat model 未改；VM 恢复初始 off 状态的验证在环境/交付记录中。
'''
    (ROOT/'docs/benchmark-overview.md').write_text(text)
    print(ROOT/'docs/benchmark-overview.md')


if __name__=='__main__':main()
