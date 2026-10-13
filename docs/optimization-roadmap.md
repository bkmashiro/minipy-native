# D current optimization pointer

D follows C (`d42e0cd`). Approved priority: a real top-level Python script entry,
then bounded performance iteration. Source names are resolved by RustPython
frontend lowering, never by a runtime name table or handwritten business kernel.

## Current baseline and candidate

C ARM64 Linux weighted 100-row cold total median: 209.29 us, compile 198.29 us;
stateful total 291.00 us, compile 278.96 us. These are independent columns.
C raw results remain in `bench/results/c-linux-v2/`; do not rerun its matrix.

The current implementation extends the one semantic IR with sequential Tool
calls, scans, output and evaluated/discarded expressions. Native code has a resume
switch only at real Tool waits and row-chunk yields. It preserves the old
`CodeUnit::compile` + `runtime::execute` entry; legacy source lowers to this IR.
`compile_script` binds a declared scalar input schema, and `execute_script`
returns status, bounded private stdout and error. No implicit expression echo.

Both operator corrections are incorporated: bool-valued expression statements
are discarded, and loop-body locals can be introduced where assigned. Reads
after a potentially empty loop require prior definite initialization. Print is
allowed inside loops but Tool waits inside loops remain explicitly unsupported.

Current verification: original runtime/lifetime tests and eight new integrated
script tests pass on macOS. Native CFG construction initially exposed an unfilled
Cranelift dispatch block and was corrected before acceptance. Frontend lane ran
on verified `gpt-6-luna / openai-codex` via Hermes, after CLI's ChatGPT account
rejected that exact model. No silent model substitution.

## Next experiment

Compare C runtime vs generalized legacy on the same old benchmark cells, then
legacy vs equivalent printing script and ordinary Monty on the same machine.
Profile fresh small-script compilation before selecting one reusable immutable
Host/ISA component. Report code/frame/layout and second-wait retention costs.
Frame currently grows from 280 to 320 bytes, plus 16-byte views and owner slots
per declared buffer. Script code that prints after a record must retain its code
until completion; terminal legacy record still drops code before waiting.

## Pending decisions and stop point

`Arc<Box<[Quote]>>` remains an isolated owner/API experiment, not an approved
change. `QuoteBuffer::downgrade` and `TestProvider::last_rows` expose Weak types.
Bytes/string language ownership, encoding and Tool ABI remain undecided.
No automatic source cache, scheduler rewrite, verifier removal or framework.
Stop at measured convergence or a genuine API/semantic decision, by 08:00 CST.
