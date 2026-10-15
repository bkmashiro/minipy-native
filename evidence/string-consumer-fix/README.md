# String consumer cost repair

Base: `7f850490fc69ac51a54ca68a845b9529a7dc9946`.
macOS ARM64, Rust 1.96.0, release, `tool-json`, one build job.

## Decision

Keep borrowed declaration validation and the private stack-formatted sink parts.
They remove specific allocations and a redundant UTF-8 pass without changing the
public string representation or adding per-task state. The request measurements
support a modest tape benefit, but do not establish stable native latency gains
or explain the older BS process drift.

The isolated allocation regression uses the same output bytes:

- Declaration validation: 12 → 4 allocation calls (−66.7%) for its fixture.
- Eight scalar pieces: 11 → 3 execution allocation calls (−72.7%).
- One scalar piece: 4 → 3 execution allocation calls (−25%).

These are allocation calls, not RSS or whole-request speedups. Numeric declaration
validation remains at four calls. The test allocator is confined to its own test
executable; speed measurements use a separate executable without it.

## Changes

`tool.patch` reuses a borrowed validator instead of constructing an owned numeric
schema solely for validation. Existing error ordering is preserved.

`runtime.patch` uses a private `SinkPart` during synchronous string consumption.
Scalars use at most 20 stack bytes. Runtime-generated JSON spans supply their byte
length directly; the eventual public `StringValue::as_str` remains checked. This
removes one full UTF-8 validation, not every validation. Whole-line output,
logical JSON scan charges, authorization and cancellation rules remain intact.
Public Host-constructed `StringValue::Json` values retain the same safe accessor.

## Request probe

`tests/string_cost_probe.rs` runs four cases on tape and native: scalar without
Tool declarations, the same scalar with an unused declaration, eight interpolated
integers, and a 16 KiB borrowed JSON string printed with a prefix and suffix.
Each case has 101 distinct sources; sample zero is recorded but excluded.
Each process owns a resident runtime and a prebuilt ready Provider response.
Timing includes per-source preparation, execution, code release and result
production. Input construction, expected-output computation, assertions, logging
and destruction of the returned output are outside the timer. No filesystem or
HTTP I/O is measured. The scalar control is not a Quote scan regression test.

Initial run order: `BVSSVB`, 4,848 rows. Confirmation: `BVSSVBVBSBSV`, 9,696 rows,
after an untimed execution of each binary to reduce the initial cold-machine
transient. B = base, V = base + tool.patch, S = base + both patches.
Every measured process starts fresh and then processes new sources. Both runs
are retained; no run or cell is silently dropped because it is slower.

The first campaign has large process drift. In confirmation, declared-scalar
tape process medians are B 5.000/3.938/3.875/3.834 µs and V
3.813/3.459/3.438/3.563 µs. JSON tape medians are B
18.792/14.771/14.791/14.729 µs and S 14.167/13.771/13.750/14.042 µs.
Native format and JSON cells are mixed across arms. Do not turn pooled differences
into a universal speedup or claim the sink change alone accounts for B→S.

## Reproduction

The two patches apply independently to the base. Overlay the committed
`tests/string_cost_probe.rs` onto that base for all three arms. Use the same
shared target directory, serial builds and the shared performance lock.
Build each arm and copy its test executable before building the next:

```sh
CARGO_BUILD_JOBS=1 cargo +1.96.0 test --release --offline --locked \
  --features tool-json --test string_cost_probe --no-run --message-format=json
# Select compiler-artifact where target.name == string_cost_probe and profile.test.
# Never select the last executable artifact: it can be the CLI on incremental builds.
/path/to/copied-test resident_source_cost --ignored --nocapture
python3 evidence/string-consumer-fix/summarize.py
cargo +1.96.0 test --offline --locked --features tool-json --test tool_validation_cost -- --nocapture
```

`identity.json` records build command, source and copied executable identities for
the confirmation run. Compressed raw contains run/arm/backend/case/sample/ns.
The baseline allocation tests fail for the expected extra allocations; candidate
tests pass. Logs are included. No speed binary is retained in the repository.

Default and tool-json full tests, tool-json all-target strict Clippy and rustfmt
passed. Independent read-only review found no concrete correctness issue.
Linux/Wasm performance and waiting RSS were not measured in this repair.
