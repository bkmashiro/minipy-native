# Excluded measurement pilot

All 300 commands completed and raw/identity/accounting passed, but the native
steady-state CPU sampling RPC was outside wall and inside CPU differences.
Its cost was material to scalar CPU (~0.59 ms); it also woke the native parent
before each request. None of these observations are pooled into formal AD v2,
including wall, first and memory rows. Not website-importable.

Preserved aggregate raw contains 12,069 rows. Original per-command raw and exact
pilot source/binary are on WSL at `/home/na/benchmarks/minipy-s/ad-probe-overhead-pilot`.
The untouched identity keys retain the original `ad-corrected` paths; for the
relocated pilot files substitute that directory with `ad-probe-overhead-pilot`.
W runtime/Guest/fixture paths have not moved. Original source commit: 37408e9.

Formal replacement: `../ad-wsl`, cohort `ad-corrected-json-prefork-v2`. Current
replay deliberately rejects this pilot cohort. It is an audit record, not a
performance conclusion. Raw values and identities have not been edited.
