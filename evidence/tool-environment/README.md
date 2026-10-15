# Reusable Tool environment

Base `fb3308c`. Add an opt-in `ToolEnvironment` that takes ownership of validated
immutable declarations. It can prepare many new sources on either backend.
The frontend borrows its table; prepared units retain only their existing bound
plans and remain usable after the environment is dropped. Runtime authorization
continues through the Provider. Input-name/type validation remains per request.

The existing declaration entry points retain validation and error ordering, but
also borrow rather than clone the whole table. Used Tool declarations are still
cloned during call binding; no new hash table, cache, scheduler or runtime tag is
introduced. The source-less environment does not cache user code.

## Measurement

macOS ARM64, Rust 1.96.0, release, tool-json, shared existing target and exclusive
`minipy-perf-build.lock`. Same binary, alternating ordinary/registered order for
each source. Four rounds per cell, 40 measured sources plus one recorded warmup.
Two table sizes (1/16), two backends, scalar with unused Tools / one ready Tool.
There are 2,624 raw rows, 2,560 included measurements. This is one process with
four paired rounds, not four independent machine trials. Source literals vary
within each round and recur across rounds; both paths prepare every request.

Timing covers preparation, execution and code destruction. Runtime construction,
Host creation, environment registration, source formatting and assertions are
outside the timer. Returned stdout destruction is also outside. The Provider
returns a ready i64, so this does not measure real HTTP or file latency.

Relative request-time changes for registered versus ordinary declaration entry:

- 1 Tool, unused: tape −7.3 to −8.1%; native −0.7 to +6.4%.
- 1 Tool, called: tape −3.9 to −4.3%; native −0.5 to −0.9%.
- 16 Tools, unused: tape −49.0 to −50.2%; native −4.8 to −5.0%.
- 16 Tools, one called: tape −33.3 to −34.5%; native −3.1 to −3.4%.

These compare APIs in the new build and isolate moving validation out of each
request. They do not measure the whole-table clone removal against the previous
binary. Absolute times drifted across workload groups; compare paired arms
within each group. Registration was recorded separately: 1 Tool 199,917 ns and
16 Tools 3,834 ns, with substantial first-call initialization in the former;
these two samples are not a registration scaling benchmark.

Keep the new opt-in entry point: it eliminates repeated fixed work and shows
consistent tape benefit in this bounded probe. No universal native speedup is
claimed. Allocation regression verifies that adding unused registered Tools
from 1 to 16 does not add per-source tape preparation allocations.

## Reproduce

```sh
CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR="$PWD/target" \
  cargo +1.96.0 test --release --offline --locked --features tool-json \
  --test tool_environment resident_new_source_cost -- --ignored --nocapture
python3 evidence/tool-environment/summarize.py
```

The raw is the `ENV` JSON lines, with the prefix stripped. The ignored probe lives
in `tests/tool_environment.rs`. Default/tool-json full tests and strict all-target
tool-json Clippy passed; the registration tests cover invalid schema, unchanged
source diagnostics, i64/string/JSON calls, use after environment drop and Provider
permission denial on both backends. No Linux/Wasm or waiting RSS run was made.
