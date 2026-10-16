# Complete-script acceptance

Run the fixed entry from the repository with Python 3 and the installed Rust
1.96.0 toolchain:

```sh
python3 tools/acceptance.py --output target/acceptance-local-01
python3 -m unittest discover -s tools -p test_acceptance.py
```

Choose a new output directory on every attempt. The entry reuses seven existing
test suites, their complete sources and payload variants. It builds ordinary
`tool-json` release test executables offline/locked, then runs every test serially.
Each suite must list its named contract sentinel and pass its complete inventory
with zero failures, ignored tests or filtering. Runtime tests select both real
native and tape backends; schema/admission tests also exercise preparation.

## Fixed coverage

| Existing suite | Complete-script and variant coverage |
| --- | --- |
| `di_corpus` | Ten public JSONL cases through the existing DI runner, native/tape and separate timing/ownership modes. Exact stdout, status and ordered Tool arguments; whole-source rejection and runtime mismatch. |
| `json_get` | Original CV05 plus complete fetch/checkpoint/publish scripts, missing/present/invalid defaults, static dot/subscript binding, UTF-8, exact bounds, true waits, last-use release and detached arguments after cancellation. |
| `json_fstring` | Original CV01, mixed JSON integer/string sinks, checked extrema, atomic errors, authority/call bounds and root liveness. |
| `json_array` | Original CV04 and ordered lazy field variants, break, malformed/reached fields, row/chunk/memory bounds, independent task cursors, wait and cancellation cleanup. |
| `line_scan` | Full count and filter/print scripts, 181 frozen CPython splitlines cases through plain and forced-escaped JSON, Unicode boundaries, >256 lines, row/scan/output budgets, break, waits and cancellation. |
| `cy_routing` | Whole-source admission before effects, preparation rejection categories, partial stdout and no fallback/replay after overflow, permission, provider, budget, input or cancellation failure; submitted effects occur once. |
| `dl_contract` | Exact UTF-8 stdout caps including newline, failed-print atomicity and later-effect suppression, scalar reassignment/static JSON binding, permission and precancellation. |

Sources and expected results remain in their existing tests, `fixtures/di`,
`fixtures/cv` and the frozen public line oracle. No private corpus is loaded or
copied. This entry does not regenerate model answers or update Agent success
statistics. Test-level counts are not numbers of independent user tasks.

## Actual Linux execution

The same entry can additionally cross-build these exact seven executables for
an existing ARM64 Linux host and execute them over SSH:

```sh
python3 tools/acceptance.py --output target/acceptance-linux-01 \
  --linux-host root@EXISTING_ARM64_HOST \
  --ssh-option=-i --ssh-option=/path/to/existing-key \
  --ssh-option=-o --ssh-option=UserKnownHostsFile=/path/to/existing-known-hosts
```

An existing `cargo-zigbuild`, Zig and installed
`aarch64-unknown-linux-musl` standard library are required. The entry does not
install them, start a VM, create a target tree or change system settings. Verify
host authorization, capacity and absence of conflicting work first. The SSH
account must have a writable `/root` directory. Add the existing HostKeyAlias
through another pair of `--ssh-option` arguments when needed. Strict host-key
checking is mandatory. The local macOS build requires the mounted external
`target` symlink and all builds/runs hold `minipy-perf-build.lock`, with one build
job. Native Linux invocation can reuse an existing local `target` directly.

Linux preflight records architecture, load, memory, disk and processes. Every
uploaded executable is SHA-256 checked on the destination before and after its
actual execution. Successful runs remove only their seven uploaded binaries and
empty task directory. A failed run retains its evidence and may leave that
specific remote directory for inspection. The VM lifecycle remains the caller's
responsibility.

`checks.json` records exact commands, source/toolchain/binary identities, named
test inventories and per-platform totals. Raw stdout/stderr accompanies it;
`complete` remains false on failure. macOS and Linux build success alone cannot
produce a completed acceptance result. DI's existing per-case timing fields are
only incidental to correctness checks here. No performance, total-memory,
service-quota, x86 Linux or browser Wasm claim follows from this entry.

## DW result

macOS arm64 and the existing ARM64 Alpine Linux VM each pass all 53 tests:
CY 5, DI 5, DL 4, arrays 13, JSON f-strings 7, JSON defaults 11 and lines 8.
Both Python entry-validation tests pass. Linux uses the same source hashes,
Rust 1.96.0 and Zig 0.15.2 cross-build, with real JIT/tape execution on the guest.
Its seven binary hashes match before and after execution.

The VM started off and is restored off after the run. It retains its existing
2-vCPU/2048-MiB configuration. Preflight finds 1868 MiB available, zero swap use
and 10.1 GiB free on the guest root; the existing external build target has
287 GiB free. No dependency downloads, new target/worktree or system changes.

See [checks](../bench/results/dw/acceptance/checks.json) for exact invocations,
identities and test names, with raw command stdout/stderr in the adjacent
`logs.tar.gz`. The production compiler/runtime, public API, dependencies,
original fixtures and existing assertions are unchanged. The stage is ready for
focused same-card review; no further optimization or model campaign is planned.
