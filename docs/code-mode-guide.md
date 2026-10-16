# Current Code Mode generation guide (DT)

Use this version for new generation against main with `tool-json`, incorporating
reviewed DR candidate `6f79919`. This is a new current guide, not a revision of
any frozen experiment prompt. Historical DP/DS guides, answers and statistics
remain unchanged. The [supported subset](../README.md#supported-subset-and-limits)
and [line contract](dr-lines.md) define the available language.

## Generate a complete script

Use only the Host's declared inputs, Tools and JSON paths. Preserve the complete
task, exact Tool arguments, source order, conditions and stdout (including each
print's newline). Do not invent APIs, hard-code replies, truncate output, move
filtering/aggregation into a Host Tool, or guess missing data. Report the concrete
gap if the task cannot be expressed. Ordinary locals are checked i64/bool;
JSON views/string-leaf aliases are single-binding. No imports, functions/classes,
general Python objects, dynamic lookups, general collections or string locals.
Use bounded string composition only at existing print/Tool sinks.

Prepare the whole source before execution. Only preparation `Unsupported` permits
the Host to consider a separately authorized executor. Other preparation errors
are rejections. Runtime errors preserve prior stdout and must never trigger
fallback or replay. Declarations do not grant permission. Keep all Host-provided
budgets unchanged; the library stdout default is 16 KiB, while a Host may explicitly
choose another policy. Do not assume the private replay Host's 256 KiB policy.

## Borrowed text lines

Use `for line in text.splitlines():` directly. `text` must be a declared JSON
string leaf or its existing single-binding alias; dot and literal subscript
selectors are equivalent. No arguments or keywords, `keepends`, intermediate
split list, dynamic receiver or general string local.

Boundaries follow CPython `str.splitlines(keepends=False)`: LF, CR, CRLF, VT, FF,
U+001C/1D/1E, NEL, LS and PS. Empty text has no lines. A trailing separator adds
no final empty line; consecutive separators retain internal empty lines.

The loop variable is a read-only temporary view. Inside its loop use literal
`in line`, `line.startswith(literal)`, bool/not/short-circuit, checked scalar
computations, if/break, `print(line)` or existing print concatenation/f-string
pieces. Initialize counts and other scalars before the loop if needed afterward.
Each print adds a newline. Use no line equality, rebinding, alias, other methods,
Tool argument escape or post-loop line access. Do not shadow an existing binding.
No nested loops, continue, for-else or Tool calls inside loops.

Existing UTF-8 validation, 65536-byte raw JSON string-token limit (including
quotes/escapes), scan budget, cumulative rows fuel, retained-buffer and chunk
cancellation limits apply. Each delivered line, including an empty one, charges
one rows unit. There is no separate 256-line cap; declared object arrays retain
their own 256-element-per-loop bound. Long single lines may execute synchronously
up to the token bound. No wall-clock preemption guarantee.

## Host/library migration

Public exhaustive matches must handle `Stmt::LineFor`, `Stmt::LineBreak`,
`Expr::LinePredicate` and print-only `StringSource::Line`. Existing public function
signatures are unchanged. The measured JSON State grows 136 to 144 B, without
an additional allocation call on the sampled old paths; old array metadata and
its exact budget boundary remain 56 B. See DR for measurement scope and limits.
