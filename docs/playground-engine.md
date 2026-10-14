# Browser interpreter / debugger package

This opt-in browser target executes the admitted Python subset through the same
`frontend::parse_with_inputs`, sealed `InterpreterUnit`, lowering and computation
tape dispatcher as the native lightweight interpreter. Browser execution is Rust
compiled to Wasm, scheduled inside a module Worker. It does not compile guest
Python to native instructions. The browser provider is a trusted bounded Quote
fixture, with real asynchronous timer completion; it is not a network capability.

## Build and consume

Heavy dependencies, targets and raw build outputs stay on WSL. Use the existing
Rust 1.96.0 toolchain and installed `wasm32-unknown-unknown` standard library:

```sh
cd /home/na/projects/minipy-u
flock /home/na/benchmarks/minipy-s/compute.lock \
  env CARGO_TARGET_DIR=/home/na/benchmarks/minipy-s/source/target \
  python3 scripts/build_playground.py
```

W also holds this lock for its measured campaign. Native default builds remain
ordinary rlibs. Only this command requests cdylib, and Cranelift/native providers
are target-excluded from Wasm. The output is `.artifacts/playground/` with
`minipy_native.wasm`, `engine.mjs`, `worker.mjs`, `samples.mjs`, small acceptance
files and `manifest.json`. The build drops Wasm custom sections (including debug
paths), retaining all executable/data sections. The manifest records actual
compiler identity, source hashes, byte sizes and the delivered Wasm SHA-256.

For a site, copy these four engine/sample files into one same-origin static
folder. The smoke files are optional. No backend, npm runtime dependency,
wasm-bindgen toolchain, CPython, JS interpreter or generated Python kernel is
required. Start `new Worker('/folder/worker.mjs', {type:'module'})` and send
`{id, op:'init'}`. Initialization fetches only adjacent Wasm. Do not expose the
optional `init.url` choice to guest source. CSP needs same-origin workers and
Wasm compilation permission. Worker termination is the hard-stop fallback.

## Worker protocol

Commands have caller-owned `id`. Replies are `{id, ok:true, value}` or
`{id, ok:false, error}`. Errors in guest execution appear in task snapshots,
not as Worker crashes. The valid operations are:

- `init`: load/instantiate once. Reply includes linear memory bytes and
  `timing.initialization_ms` (fetch + compilation + instantiation).
- `prepare`: `{source, inputs:[[name,'i64'|'bool'],...]}`. Returns `{unit,tape}`;
  each tape entry has actual `pc` and sealed-op description. Preparation uses
  the same frontend and admission bounds as native.
- `create`: `{unit, inputs:['2',...], limits?, provider?, batch?, trace?}`.
  Returns `{task,snapshot}`. All i64 input/value/result fields use decimal
  strings to preserve full precision in JS. Bool inputs use `'0'`/`'1'`.
  Multiple tasks share one immutable prepared unit.
- `run` / `pause`: resume/stop cooperative dispatch globally. Pausing does
  not stop already-launched provider operations; they can complete into a
  ready task while paused, without executing the next instruction.
- `step`: `{task}`. Pauses automatic dispatch, executes at most one actual
  tape instruction, with persistent scratch across stops. A Tool instruction
  launches the fixture provider and enters a wait. Stepping a waiting or
  terminal task does not advance it.
- `advance`: `{task,count}`. Manual debug batch of 1..2048 instructions,
  stopping earlier at Tool/chunk/terminal boundaries. It also pauses run.
- `snapshot`: `{task}`. Returns current engine state without execution.
- `cancel`: `{task}`. Stops that task, releases all input owners, retains
  earlier stdout and discards a late provider result. Other tasks continue.
  A launched provider remains detached until its timer completes.
- `reset`: clears prepared units and tasks, stops dispatch and increments
  `epoch`. Old provider deliveries cannot attach to newly reused task IDs.
- `ack`: `{update_id}` without a reply id. Acknowledge rendered/ingested
  update batches, then the Worker can continue scheduling.

Execution sends `{type:'update', epoch, update_id, task, events, snapshot,
linear_memory_bytes, timing}`. Ingest each batch once and immediately send its
ACK. The automatic scheduler stops at two outstanding update batches; at most
eight already-launched provider completions can additionally arrive. Manual
step/advance/cancel requires fewer than two outstanding updates. Step/cancel
replies repeat their update contents for command confirmation: render events
from the update channel only, not again from the reply. Event IDs are monotonic
within an engine epoch; task IDs and event IDs reset with the engine. Ignore old
epoch updates and clear frontend playback/selection on reset.

`provider` has `{rows:512, delay:25, price:'7', quantity:'3', fail:false}` defaults.
Rows are immutable Quote/v1 records built by Rust at actual Tool completion.
Record echoes the submitted total. `delay` is 0..5000 ms and `rows` is
0..1,000,000. Prices and quantities are checked i64 values. The guest cannot
select arbitrary tools, URLs, host paths or options. Fixture import validates
aggregate available input bytes before allocation. Provider failure, permissions,
scan fuel, stdout cap and checked arithmetic stay task errors.

`limits` may narrow `rows`, `chunk`, `buffer_bytes`, `tool_calls`, `stdout_bytes`,
`allow_load`, `allow_record`, `allow_print`. Defaults are existing ScriptLimits:
1,000,000 total scan rows, chunk 256, input bytes 16,000,000, 512 Tool calls,
stdout 16,384 bytes and all three capabilities enabled. Browser ceilings match
those defaults, with chunk 1..4096. Engine bounds are eight task slots and 16
prepared units per reset. Source/node/local/buffer admission is unchanged:
16 KiB source, 512 nodes, 32 scalar and 32 buffer slots. These are engineering
bounds, not browser platform maxima. Commands are capped at 64 KiB JSON and the
built Wasm memory maximum is 256 MiB. Tasks/units have no automatic eviction.

## Trace, ownership and memory rendering

- Snapshot state is `ready`, `load_wait`, `record_wait`, `done`, `error` or
  `cancelled`. `pc` is the next instruction, `index` the scan cursor, and
  `remaining` reserved scan fuel. Locals are numbered scalar slots, not
  fabricated source-name bindings; the fixed frame includes unused slots.
- Events include `instruction`, `yield`, `load_wait`, `record_wait`,
  `tool_resume`, `done`, `error`, `cancel`. They come from actual dispatcher
  and capability completion paths. `detail.execution` carries executing PC,
  next PC, stdout delta, IR instruction range, actual Quote-row copy range,
  and scalar/scratch writes. Successful single-instruction writes include
  unchanged values. Tool resume includes a record-result write when applicable.
  This is a debugger projection, not a log of every allocator or CPU memory access.
- `frame.offset/bytes`, `frame.locals_offset`, `tape.offset/bytes/stride`,
  `buffers[].offset/bytes/rows/slot/preview` are actual live Rust pointers/sizes
  in Wasm linear memory. Event ranges are actual offsets. Tape is IR code,
  not native machine instructions. The preview contains at most four rows.
- Buffers belong to their task; tape belongs to the shared prepared unit.
  Last-use release at bounded execution boundaries uses the same liveness
  mask as native. Done/error/cancellation releases all buffer owners.
  Terminal task frames and prepared tapes remain as bounded debugger slots
  until reset. Do not portray them as freed. The logical frame/tape/payload
  ranges omit owner headers, unused Vec capacity and allocator metadata;
  they are not a complete heap allocator map.
- `linear_memory_bytes` is Wasm capacity, not live allocation bytes. Reset and
  owner release need not shrink it or return pages to the OS. Distinct sparse
  addresses must remain distinct in the visualization. Never translate a
  layout coordinate into a claimed runtime address.
- `batch` defaults to 1024 and is capped at 2048. Debug trace defaults on.
  Every batch stops at a real chunk yield or Tool boundary; UI updates are
  not dispatched per instruction. Frontend playback should use a bounded
  ring (e.g. 4096 events), retaining a visible dropped-history count and
  latest snapshot. The engine and Worker keep no event history.
- `trace:false` uses the same fast `run::<false>` specialization, reporting
  boundary events and final state deltas only (`granularity:'boundary'`).
  It must not be displayed as an instruction-complete trace. The native
  default has no debug module, persistent scratch, event serialization,
  step checks or debugger allocation; the STEP branch is feature/constant gated.

`timing.command_ms` is the measured ABI call including JSON encode/decode and
engine work for that command. Preparation timing is on the prepare reply;
advance/step timing includes debug projection costs. Provider delay, scheduling,
transport and frontend rendering are separate. These are browser diagnostics,
not native benchmark results or clean dispatch-only scores.

## Verification

Run native correctness plus the optional debugger tests:

```sh
cargo test --tests
cargo test --features debug-runtime --test debug_runtime
cargo clippy --features debug-runtime --lib -- -D warnings
python3 -m http.server 8769 --bind 127.0.0.1 --directory .artifacts/playground
```

Open `/smoke.html`. Its actual module Worker/Wasm test exercises source edit,
retained scratch and PC stepping, three independent tasks, real async wait and
resume, row/chunk events, input release before record wait, cancellation/late
completion, permission/provider/fuel/output errors, overflow, i64 precision,
bounded traces and ACK backpressure. It exposes `window.__qa` for automated
inspection and terminates the Worker on completion. This is an acceptance page,
not the future V workbench. UI design, Canvas playback and deployment belong to V.
