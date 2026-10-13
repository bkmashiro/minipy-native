# Quote benchmark

The binary compares source-compiled MiniPy Native with pinned in-process Monty. The original `speed`/`none` campaign also includes fixture-specific handwritten floors (`monty_fixture_helper`, `host_fixture`). The corrected `generic` mode uses one parameterized Quote/v1 filter/product/sum primitive with fields and thresholds decoded from the actual Monty call. Ordinary Monty receives typed tuples and uses two host boundaries; the generic helper uses an opaque typed-buffer handle and three boundaries. No CPython, JSON or IPC path is used. All successful receipts are checked against a semantic oracle.

Run from repository root:

```sh
rustc +1.96.0 --version
cargo +1.96.0 run --release --manifest-path bench/Cargo.toml --bin quote-bench
cargo +1.96.0 run --release --manifest-path bench/Cargo.toml --bin quote-bench -- generic
python3 tools/run_generic.py
python3 tools/summarize_generic.py
python3 tools/summarize.py
```

Rows are 0, 100 and 10,000, with 20 raw samples per engine/cold-warm/chunk cell. Source-cold totals include producer construction, compilation, conversion, execution and cleanup. Compile-once warm execution is explicitly separate. Ready provider operations do not sleep. Separate million-row scheduling controls and cancellation probes have their own timing scopes. Native task-owned quote/frame payload is zero at the first and second waits; future bodies, channels, provider and compiler resources are not included in that payload counter.

## Evidence and limitations

Published Monty 1.1.0 failed to compile because it iterates over a boxed array without dereferencing it. The dependency is pinned to upstream commit `56949a4c45f51cedc7a96450782f58a8017e114d`, which already fixes this issue. No local source patch is used. `results/` contains actual macOS and Linux ARM64 runs, 5,976 verified samples, source/toolchain identity and whole-process CPU/RSS evidence. `../tools/run_bench.py` reproduces local or optional existing-VM runs. Full commands, attribution, negative results and unmeasured quantities are in `../docs/experiments.md`.

Weighted native cold execution loses to the generic helper. That stateless primitive does not support the cross-row stateful case; its earlier handwritten helper numbers are fixture performance floors, not general-helper evidence. The bounded rerun adds 960 checked samples in `generic-{macos,linux}.csv`, a separate identity and 48 summary groups; prior raw data/identity remain untouched. Machine-code byte counts are not physical JIT memory footprint; whole-process RSS does not establish per-engine or per-task density. Trace and memory observations are separate from timed single-invocation samples.

The comparisons are not safety-equivalence claims: Monty supports arbitrary-precision integers and has different execution/resource-budget semantics from this native i64 prototype. The workload is bounded synthetic data, not a production wait distribution.
