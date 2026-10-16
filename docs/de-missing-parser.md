# DE: bounded missing-key parser proof

## Result and scope

The DD candidate is feasible with the current pinned safe dependencies for the
bounded cases below: resolve a required parent separately, allow only a single
final object key to be optional, treat NotFound as a candidate, then validate the
selected receiver's UTF-8 and JSON structure before accepting absence. This is
parser evidence, not an implementation or approval of `get`, missing-only
validation policy, or a public `StringSource` variant.

Baseline: reviewed DB `0b38696ada4e748b8c43a0b8e95c8183e13dd3fc` (signature G).
Only `tests/de_missing_parser.rs`, this brief and the experiments checkpoint
change. No production source, manifest, lockfile, API, dependency, backend or
source-language behavior changes. No benchmark, CPython substitution, new parser,
unsafe probe, target directory or worktree. Six dependency-level tests pass;
15 existing Tool/JSON integration tests also pass, including real native/tape
execution of the existing lazy contract. This does not execute source-level get.

## Observations from actual Rust execution

- Empty and normal objects produce final-key NotFound; present empty string,
  scalar, null, array and object tokens remain Found, never absence. Duplicate
  and escaped keys select the first match, including a first null.
- A string key on a non-object is not NotFound. Multi-key lookup cannot distinguish
  missing parent from missing final key. A separate required parent lookup is
  essential. Index NotFound also includes empty/out-of-bounds arrays and is not
  evidence of an object. Non-object selected parents remain errors.
- `{garbage}` returns sonic NotFound for `owner`; independent validation rejects
  syntax. Bytes `{"other":"\xff"}` (literal invalid byte 0xff, not a JSON escape)
  also return NotFound; explicit UTF-8 validation rejects them. These counterexamples
  are retained as assertions, not accepted missing outcomes. `IgnoredAny` from
  bytes alone accepts the latter, proving the UTF-8 step is required.
- Safe empty-path get borrows exactly the receiver object, excluding leading
  whitespace, trailing whitespace and external garbage/comma/invalid-UTF8/truncated
  JSON. For ` \t\r\n{"other":1} ` plus each bad suffix, the checked range is
  `4..15`. Pointer subtraction and start/end bounds are checked against the
  immutable owner and the current parent range. Selecting a nested complete
  `meta` object also ignores a malformed outer suffix and missing outer close.
- Bad delimiters, missing colon, truncation, bad literal, trailing comma, invalid
  escapes/hex, short Unicode escape and literal control characters within the
  selected object fail the independent validator (or fail safe selection first).
  Invalid UTF-8 in a selected value/key fails explicit validation and empty-path
  selection. External bad suffixes are deliberately not fed into the validator.
- IgnoredAny accepts syntactically complete surrogate escapes in unvisited values:
  paired U+10000 and U+10FFFF, lone high/low, reversed pair and high-plus-text.
  Actual sonic String decoding accepts the valid pairs and rejects the others.
  Thus this is structure plus input-UTF8 validation, not conversion of all leaves
  to the runtime String domain. A validation-only lone-surrogate key is also
  accepted by IgnoredAny; this says nothing about sonic key decoding succeeding.
- Existing direct present lookup succeeds before a malformed sibling, invalid
  UTF-8 sibling or missing receiver terminator. The proof does not apply strict
  validation on that path. Existing required `[]` still errors on missing; a
  selected containing object can reject internal malformed siblings before its
  leaf is demanded. The existing missing-outer-terminator/count=10 case remains
  successful. `tests/tool_json.rs` independently exercises both actual backends.

No tested required condition failed. The first probe compilation failed because
five generic sonic calls received byte-array references rather than slices;
explicit `.as_slice()` fixed only the test's input types. No behavioral assertion
was weakened, ignored or removed to obtain the passing run.

## Work, allocation and limitations

This is not a single-pass or allocation-free method. A new root missing proof can
perform final-key scanning, safe empty-path structural selection, that successful
get's UTF-8 pass, an explicit UTF-8 pass and serde's structural pass. An existing
parent may already have been selected/scanned; validating its cached exact range
can avoid another empty-path selection. These are source-derived traversal costs,
not measured timings, and some scans may stop early on errors. Present/required
paths need no new work. Repeated absence has no proposed cache in this proof.

There is no retained DOM: the result is `IgnoredAny` (unit-like), not Value, a map
or strings; serde_json's `de.rs:1102-1214` iteratively skips tokens and uses its
scratch Vec for enclosing container frames. This supports O(nesting depth)
transient scratch, not zero allocation or a fixed parser memory quota. sonic's
`parser.rs:1801-1819` creates a 128-byte initial key scratch Vec even for an empty
path; escaped keys can grow it. Error construction can also allocate. Exact
allocation counts/peaks, deep/adversarial-input costs and end-to-end latency were
not instrumented. Input byte bounds remain necessary; parser transients are not
shown to be charged to the task's retained buffer budget. No Linux execution,
performance campaign or universal malformed-input equivalence is claimed.

Pinned source explains the counterexamples: sonic `error.rs:69-99,122-128`
classifies NotFound; `parser.rs:1672-1704` searches its initial key/end token;
`lazyvalue/get.rs:403-414` returns errors before the UTF-8 postcheck. serde_json
`read.rs:625-645,1076-1098` skips strings/escapes without validating UTF-8 or pairing
surrogates. The explicit from_utf8 check closes the byte-validity gap without
forcing String decoding on unrelated values.

## Reproduction and compact raw evidence

Host: macOS 26.4.1 (25E253), arm64. rustc 1.96.0
`ac68faa20c58cbccd01ee7208bf3b6e93a7d7f96`, LLVM 22.1.2,
`aarch64-apple-darwin`. Existing stable rustfmt 1.8.0-stable
`01f6ddf758`. Pinned sonic-rs 0.5.7, serde 1.0.229, serde_json 1.0.154.
Before building, no other relevant writer/build process was found and `/Volumes/M.2`
was mounted; existing `target` symlink resolves to
`/Volumes/M.2/caches/minipy-native-target` (287 GiB available at inspection).

Both Cargo test invocations and final fmt ran under an exclusive nonblocking
fcntl lock on `/Users/yuzhe/.hermes/cache/scratch/minipy-perf-build.lock`, with
`CARGO_TARGET_DIR=/Users/yuzhe/projects/minipy-native/target`,
`CARGO_BUILD_JOBS=1`, `RUSTUP_AUTO_INSTALL=0`. No downloads or extra builds outside
that target. From the repository root, reproduce the final locked check with:

```sh
python3 -c 'import fcntl, os, subprocess
lock=open("/Users/yuzhe/.hermes/cache/scratch/minipy-perf-build.lock", "a")
fcntl.flock(lock, fcntl.LOCK_EX|fcntl.LOCK_NB)
os.environ.update(CARGO_TARGET_DIR="/Users/yuzhe/projects/minipy-native/target", CARGO_BUILD_JOBS="1", RUSTUP_AUTO_INSTALL="0")
for cmd in [["cargo", "+1.96.0", "test", "--offline", "--locked", "--release", "--features", "tool-json", "--test", "de_missing_parser", "--test", "tool_json"], ["cargo", "+stable", "fmt", "--all", "--", "--check"]]:
    result=subprocess.call(cmd)
    if result: raise SystemExit(result)'
git diff --check
git diff --exit-code 0b38696 -- src Cargo.toml Cargo.lock
```

First successful isolated run additionally used `--test de_missing_parser --
--nocapture`. Its raw diagnostics:

```text
false-missing [123, 103, 97, 114, 98, 97, 103, 101, 125]: sonic=NotFound, validator=syntax
false-missing [123, 34, 111, 116, 104, 101, 114, 34, 58, 34, 255, 34, 125]: sonic=NotFound, validator=utf8
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Final combined run (separate test binaries):

```text
de_missing_parser: 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (0.00s)
tool_json:        15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (0.12s)
cargo +stable fmt --all -- --check: exit 0
git diff --check: exit 0
production src/Cargo.toml/Cargo.lock diff from DB: empty
```

SHA256 identities:

```text
Cargo.lock                  15a236e43daf854bedbd6a83a6a667d1e7a173e9100008be03d82efbc3efe97e
probe                       50207ab4d646e8561cdae593db0d10f56f6f319a8bb4b625e86f19fdd318ec27
DD report                   253e28af9cd96125adada8c9ec775ab9d858dea9a4ada118293336e9b37e2162
sonic 0.5.7 src/parser.rs   c60ccbf386b5d3c88b4ed78edfcc91f12e55804f8cecbc78e7e7b1591809e3cc
serde_json 1.0.154 src/de.rs cb55666913c47d86130e7bd48a9c35081ea9a4bb0e2c45ab6429912d905629c5
```

## Minimum next gate

Review this proof first. The controller still owns the decision whether to accept
missing-only whole-selected-object validation and its extra scans/transient
scratch, plus any source-breaking public enum variant. Only after explicit
approval should a separate implementation integrate shared print/Tool get sinks
and their native/tape budget, ordering and lifetime tests. No follow-up card or
implementation is created here.
