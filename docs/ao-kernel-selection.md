# AO：高频操作筛选与 kernel 准入结论

## 结论与交付点

本轮不新增 kernel，不修改生产代码。交付可复跑的具体操作聚合、六项候选排序、实际双后端编译边界与下一实验前置设计。没有开始性能 spike，也没有端到端加速或性能负结果：选型门没有找到同时具备已证需求、现有值契约和明确可删工作的候选。任务允许这种情况下先交付设计，不能用猜测类型或省略适配的 microbenchmark 代替。

高频候选主要需要字符串、集合或结构化输出契约。现有 JSON i64 多字段访问已经可执行，prefix/leaf cache 已存在；再做 eager 批量提取会越过按需错误契约。Quote 归约能够使用现有类型，但样本没有建立这些 `sum` 的 Quote 来源或 i64 类型，更未建立它们的执行 CPU 占比。没有据此把 sum 预定为首个 kernel，也没有重跑 AH 数组实验或 AL 调度调参。

基线：独立 `exp/ao-kernels` 从初始 c3dff6a fast-forward 到已批准 AN `79bfcf88bc7826473195d6bb4c21fb7342e9a39d`。其祖先含 AH f7167add、AJ a780931、AL ea949aa。看板交付与批准状态已核对。没有 merge main、覆盖其他分支、改网站或冻结 bench；本轮仅新增工具、测试、聚合和文档。

## 来源、计数口径与复现

复用本机私有 Z 的 `survey.py`，不复制提取器或语料，不执行轨迹源码。原 cutoff=1791981392、message high-watermark=5801931、各来源时间分位抽样、session+call-ID replay 去重和 exact UTF-8 source 去重均保持。新分析只在原提取器的 `analyze` 上附加固定 AST 计数，不建立通用匹配框架。

复跑时在内存核对原 sample session hashes、输入文件 hashes、replay 统计，以及每组完整原 `unique_exact` 聚合、原提交数、零截断；全部 exact-match 才输出。主样本仍为 Hermes 577 与 AppWorld 55，跨来源632不同脚本。浏览器 stratum、shell bodies、仓库源码不进入主分析。相同脚本跨 Hermes 来源再去重一次后得到577，而非简单相加各组。每项频率是“一个不同脚本至少出现一次”；类别可重叠，不是动态调用数或热点时间。

固定名 `json.loads`/`json.dumps`、`sum`/`all` 仍可能被重新绑定；method receiver 和索引对象没有推断类型。相同 receiver 的字面路径按语法合并，可能跨 scope、重赋值或循环；不能当 alias/def-use 证明。literal key read 只算 Load，不算 Store，不包括数字/动态索引。嵌套路径的内部前缀也是一个访问，重复前缀与重复叶均计入 repeated path。分析器没有去掉不可达代码，脚本出现频率也不说明成功运行。结果不代表完整脚本兼容率。

去值聚合保存于 `evidence/ao-kernels/operations.json`，含每来源及跨来源 family 的 presence/occurrences。没有源码、参数、键值、标识符、会话 ID 或逐脚本 hash。原提取器临时写出的私有聚合 metadata 位于本工作区临时目录，退出时删除。最终仅保留提取器、原 summary 和分析器自身的 SHA256。

## 具体操作证据

| AST 形状 | Hermes /577 | AppWorld /55 |
|---|---:|---:|
| `json.loads/load` | 148 | 0 |
| `json.dumps/dump` | 202 | 0 |
| 相同 receiver / parent 路径的至少两个字面 key | 107 | 6 |
| 同脚本有 decode 与上述多 key 形状，非数据流证明 | 84 | 0 |
| 字面 key 读取 / 嵌套 key 读取 | 188 / 25 | 15 / 0 |
| 重复字面路径，含重复前缀 | 75 | 0 |
| 字面 key 读取被绑定 / 直接传给任意调用 | 12 / 83 | 6 / 0 |
| 字面 key 读取直接传给 JSON encode | 6 | 0 |
| filtered comprehension / field projection | 71 / 57 | 7 / 5 |
| `all(generator)` / `any(generator)` | 44 / 7 | 0 / 1 |
| `next(generator)` / `len(listcomp)` | 5 / 1 | 3 / 0 |
| `sum(generator)` | 27 | 0 |
| sum generator 字段项 / predicate 形状 / 其他项 | 18 / 8 / 1 | 0 / 0 / 0 |
| 循环内 `+=`，未判断数值或字符串 | 12 | 2 |
| 字面 needle 的 membership，haystack 未定型 | 25 | 0 |
| `startswith` / `endswith`，receiver 未定型 | 9 / 7 | 0 / 0 |
| `splitlines` / `split` / `rsplit` / `partition` | 16 / 9 / 2 / 1 | 0 / 0 / 0 / 0 |
| `join`，本样本 receiver 均为字符串字面量 | 31 | 0 |
| `index`，receiver 未定型 | 9 | 0 |
| `find` / `rfind` method | 0 / 0 | 0 / 0 |

独立 `filter(...)` 名称调用为0，`map(...)` 为Hermes2；filter/project 的依据主要来自推导式，而非这两个 builtin。AppWorld 的普通属性访问很多，本表的 literal-key 数量不能代表其全部字段读取。`p[key]` 的存在、`json` 的共现不能证明同一对象来自 JSON；子树赋值/传参也可能是 scalar，不将它们重命名为“已确认 raw subtree passthrough”。原 Z 字符串类别576、索引286、json任意调用271不是这些 kernel 的频率。

## 六项排序：净工作与当前门槛

优先级是下一步研究顺序，非性能排名，未把网络等待算作计算收益。

1. JSON decode 后多字段读取 / 只读子树直通。
   需求线索：多 key 107/6、decode共现84/0、子表达式传 encode 6/0。可删除的工作是完整 DOM 的字段/字符串分配、无关字段解码，以及真正重复的 sibling 扫描；sonic safe get 和 typed decoder 内部已经有成熟解析 kernel，不能重复记作新增收益。
   新增成本：准备期绑定 schema/path、范围与解码 scratch、owner 保活、字符串转义或最终输出验证。AH 已支持静态 i64 leaf 和 raw alias，已删掉部分 DOM 工作；alias 不是可传 Tool/输出的 JSON 值。扩展到直通需新的参数/输出契约。直接 get_many 会提前访问未要求字段；新 sibling cursor 还需证明 duplicate-first、转义与畸形输入错误边界，且已有 AH 全数组 typed decode 优于 lazy/row-slots 的负证据。当前不重新实现 cursor 或预取融合器。
2. 有界集合 filter/project，最终按需求标量聚合或输出。
   线索71/7 filtered、57/5 projection。流式执行可能删除 Python 行对象、predicate/field dispatch 和中间列表。单独把 predicate 移进 helper 并不自动删除结果物化；消费者真需要集合时仍要付结果空间。
   新增成本：静态元素 schema、进入循环的数据适配/解析、输出行分配或受限 borrowed projection、bytes/fuel/chunk 记账。现有只读 Quote 扫描已有这种无行对象执行；真实样本的动态 list/dict/属性对象无法直接导入它。需要只读 collection/record 契约，而非任意对象方法或动态逐行 helper。不建立 kernel 组合或数组流水线。
3. `all/any/next/count` 的短路/计数遍历。
   线索all44/0、any7/1、next5/3、len-listcomp1/0。可删除 generator dispatch 或不必要结果集合；any/all/next 本身已经是 CPython builtin，仍逐项调用 Python predicate；没有把一个 builtin 调用全部记作可消除成本。
   新增成本：受限纯 predicate 的准备绑定、输入解码/遍历、next 的结果/缺省类型与 owner。Quote + i64/bool 的某些循环可承载，但无样本到该 schema 的证据；普通返回对象、truthiness、StopIteration/default 目前缺契约。不得为更快结果提前检查后续元素或取消 chunk/fuel。
4. 字符串 contains/startswith/分隔位置。
   线索literal needle25、startswith9、index9、split-family。成熟 Unicode/string 搜索已经在库内，新增 kernel 主要可能省解码后的重复分配、解释分派或无必要 split 列表，不能声称重新发明搜索更快。
   新增成本：字符串来源及 UTF-8/JSON escape 解码、literal 绑定、borrowed range、字节扫描预算。Rust `str`/`memchr` 类成熟实现优先，无 ASM 理由。当前源语言不能表达字符串值，Tool ABI 只能传至多两个 i64。Unicode 位置与 Python index 异常、tuple prefixes、split 空白语义等也不能用 byte offsets 冒充。先定义一个窄值契约，不能借优化名义吞下整个字符串库。
5. join 与 JSON 输出序列化。
   线索literal join31、JSON encode202。这些调用已经在 C/Rust 库中做批处理；进一步收益依赖删除中间字符串/列表、重复序列化或把字节直接交消费者。仅替换一次 library call 的可省工作可能很小。
   新增成本：长度预估/溢出与输出 cap、一次最终分配、escape、遍历检查、Tool bytes/structured output 的 ABI。当前只有 scalar print 和 i64 Tool 参数，连结果去向都未接通；不能把序列化成本移到计时外，也不能把原始 JSON raw bytes 默认当作 `json.dumps` 的等价规范输出。
6. checked i64 归约。
   线索sum27，其中字段项18仍未定型，predicate项8可能是计数；循环 `+=`12/2也可能是字符串。可删 tape per-row 运算/分支分派和 generator overhead；不能删除 Python 任意精度到i64的适配，亦不能把普通sum自动称作checked。
   新增成本：识别/准备明确的自然 builtin 或极小纯循环、typed row import、checked 累加和chunk continuation。Quote型手写循环已支持，新增sum语法和预编译内核技术上可做，不需要通用Value；但没有当前数据来源上的需求/净收益证据。原项目已有强参数化typed helper对照，不能再次拿弱逐项路径制造优势。短路与逐项溢出顺序禁止 reassociation/SIMD reduction；不预定sum胜出。

## 实际准入检查与未做的测量

`tests/kernel_admission.rs` 的一项测试向真实 tape/native 准备入口各送14个合成程序：多i64字段和raw alias两种接受；raw输出、json encode、contains、startswith、split、join、filter/project、any、all、find、count、sum十二种拒绝。它证明的是这14种具体写法，不是所有等价循环都不支持，也不是14个完整执行任务。Quote手写for仍可运行，未来新增builtin时应同步更新这个边界测试。

实际运行19项针对性Rust测试：新增准入1、既有tool_json14、真实loopback HTTP4。后18项执行既有native/tape链，覆盖pool/permit、missing/null/type/duplicate/escape、错误与短路顺序、checked溢出、输出前缀、buffer cap、跨等待owner/取消与late completion。没有削弱测试。8项Python合成测试验证固定计数、membership方向、prefix重复、未定型归约、值/标识符不导出及源码不执行。聚合两次复跑逐字节一致。新增Rust测试strict Clippy和fmt通过。

本轮没有候选执行代码，因此没有候选/基线时间、大小输入命中率、allocator、RSS/PSS或Linux结果。没有把上述测试耗时当性能数字，也没有为“首个实验”硬接一个忽略字符串/容器/输出适配的Rust答案。现有错误/lifetime测试用于边界资格，不能替代未来kernel的差分与source-to-result测量。

## 下一实验建议，需主控决定值契约后再开工

先做声明式只读 JSON UTF-8 leaf → 单一字符串谓词 → bool 输出这一窄切片的可行性设计，优先借用现有owner/path与自然 `in`/`.startswith` 写法，不引入全局动态Value、通用list、运行时selector或JIT。它不是本轮已经选定/实现的kernel；先确认需要这种Host数据形状，才能选contains或startswith其中一个。若Host可直接返回所需bool，保留这个强简单对照，避免无意义地在guest重做解析。

具体前置决定：
- leaf schema区分i64与只读字符串，明确null/type、JSON unescape、UTF-8与重复key；不可通过把string谎报为i64试跑。
- 字符串范围/decoded scratch的所有权、跨Tool等待最后使用与聚合bytes上限；不用字符串的任务不增加状态。
- 字节扫描的单helper工作上限与取消/让出边界。现有row fuel不能直接代表字符串CPU；先定资源契约，再决定是否需要chunk continuation。
- 仅支持两个明确字符串操作数的一种自然谓词，不承诺tuple prefixes/任意方法。bool输出复用现有print，不增加structured stdout或bytes Tool ABI。
- 一旦能用共同source正确执行，对比现有成熟库/Host直接bool与guest适配，计入source解析/绑定、JSON解码、数据导入、执行、stdout消费和cleanup；prepared仅另列。小/大输入、首部/末部/未命中、escaped/non-ASCII、missing/null/类型错和取消均需覆盖。有限AB/BA进程配对，无净收益撤回，不扩成第二项kernel。

这比立即建立kernel注册表、代码组合器或全面字符串系统更可控。未自动创建开发后继卡；本轮交主控选型。

## 复跑与资源

本机macOS26.4.1 arm64/Apple M4，Python3.14.7、rustc/cargo1.93.1。复用唯一已有target，offline/locked、一个构建job，无下载/WSL/VM/全量benchmark。原始输入保持本机私有。验证日志：`evidence/ao-kernels/validation.txt`；身份与资源：`identity.json`。没有生产依赖、runtime、Frame或默认backend改动。

    export CARGO_TARGET_DIR=/Users/yuzhe/projects/minipy-native/target CARGO_BUILD_JOBS=1
    python3.14 -m unittest discover -s tools -p test_ao_kernel_survey.py -v
    python3.14 tools/ao_kernel_survey.py --private-dir "$HOME/.hermes/private-research/minipy-trajectory-survey" --output evidence/ao-kernels/operations.json
    cargo test --offline --locked --all-features --test kernel_admission --test tool_json --test http_tool -- --nocapture
    cargo clippy --offline --locked --all-features --test kernel_admission -- -D warnings
    cargo fmt --check

私有提取器与summary是复跑所需的本机输入，未提交语料。其哈希不同时必须重新核对，不能把不同样本数字混入本轮。签名独立分支交定向审查，不自动合并或发布。
