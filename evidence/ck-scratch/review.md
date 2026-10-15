# CK 固定候选独立审阅与最终撤回

目标 `6e229fd88fa0f57b1bf0cfba382d363ee92bb9f3`，基线 `049b8102509d2d687912f0898ef1eda63bb81b14`。

独立 reviewer 为 `gpt-6-luna` / `openai-codex`，Hermes one-shot session `20261016_071242_35f35d`，exit0。
主控回读本机 `session_model_usage`，task为空的6次主推理调用均为精确上述model/provider。
另有1次 `gpt-5.6-luna` title_generation，仅标题辅助，不计review推理；没有模型静默替换。
命令使用 `--ignore-rules --oneshot --model gpt-6-luna --provider openai-codex --reasoning high --max-turns 16 --run-budget 240`，
移除继承的Kanban task/workspace/branch环境，明确禁止reviewer编辑、构建、测量或关闭卡。
CLI有 `Unknown toolsets: messaging` warning，session仍正常完成6次主推理和实际只读工具调用。

结论：无实现正确性阻塞，建议撤回。

Reviewer核查高水位覆盖、两条安全完整初始化数组路径、Load/Record/chunk边界和持久debug Frame。
逐实际反汇编核对 `_bzero` import、四条vector pair store、wrapper/callee栈和完整request计时边界。
通过拦截 `Path.write_text` 只读重放reducer，captured summary与固定commit文件完全相同，
source hashes由Git blob+保存patch重建，固定范围diff check通过。未重新构建、测试或计时。
原release suites/Clippy/fmt执行记录是主控证据，不能称为独立重测。

发现一项非阻塞限制：oracle仅在effect/Next清空write集合，未建模普通statement间槽索引重用，
可能沿用前一语句的写入，故不能将它视为完整语句级读前写证明。已缩窄README和experiments的主张。
候选每turn均完整初始化可访问数组，不依赖oracle保证内存安全。主控没有继续修补将被撤回的方案。

主控采纳revert，移除高水位、slice参数、small/fallback分支及候选专用tests。
最终 `src/`、`tests/`、Cargo manifests与基线逐字节一致，CJ保留不动；完整候选及tests仍可由压缩patch重建。
回退后重新运行完整default和tool-json/debug-runtime release suites、严格all-target Clippy、fmt、9 Python tests，
全部通过，见 `final-verification.json`。全部Cargo/build/测量持原fcntl锁、已挂载外置target、offline/locked/单job。
没有追加正式样本、第二候选、unsafe、Tool特化、新依赖/下载或平台执行。

审阅后在锁内逐binary SHA256匹配raw manifest，删除两份formal binary和两个sidecar，
逻辑长度合计21,444,300 B；不声称物理回收量。既有共享target/toolchains保持。
最终结论是结束此初始化路线，保留基线；没有通用性能、cache/RSS、生产Code Mode收益主张。
