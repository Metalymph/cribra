# Streaming core contract

Cribra 0.5.0 adds an opt-in synchronous Rust session. `Scanner::stream()` uses
unlimited optional limits; `stream_with_limits(StreamLimits)` applies explicit
per-source limits. Immutable compiled rules are shared with the scanner.

`push(&[u8])` accepts fragments in source order. Input storage may immediately be
reused. UTF-8 scalars and CRLF may cross fragments. Empty fragments are no-ops.
`finish(self)` consumes the session, validates genuine EOF, completes matcher and
pending validator work, normalizes accepted records, suppresses overlapping
structural candidates, and returns the existing `ScanReport`.

There is no pre-EOF result API, source-sized fallback, excerpt retention or new
binding surface. Applications own I/O, source identity and orchestration. Report
fields, serde representation, metadata, scalar coordinates and whole-source scan
methods retain their existing meanings.

## Admission and terminal errors

Admission precedes source execution allocation. Every matcher and semantic gate
must pass: regex plans must be bounded, conditional absolute anchors and mixed
LF/CRLF contexts remain unsupported, per-rule regex prefilters are rejected, and
prefix validators must require no candidate bytes. Suffix needles must be ASCII
token bytes, at most 4096 bytes, and have no proper prefix/suffix self-overlap.
Whole-source and streaming suffix matching both reject oversized tokens.

All validator requirement classes are supported for admitted configurations.
Validation uses the authoritative dispatch with exact candidate bytes and clipped
bounded context. Coordinates are captured during emission, including deferred
validation and long-lived prefixes. No candidate source text survives in output
records.

Admission returns an explicit error instead of fallback. Any execution error is
terminal: working output and retained source state are dropped, and subsequent
`push`/`finish` returns `NotActive`. The first execution error takes precedence
when transport validation later fails in the same push. Incomplete UTF-8 at EOF
returns an error. No partial output is presented as a completed report. Dropping
buffers is not a secure-zeroization guarantee. Session Debug and errors are
redacted and do not contain retained source text.

## Three distinct limits

All limits default to `None`; existing whole-source behavior is unaffected.

- `max_candidates`: cumulative raw rule emission count, checked before ingestion.
  Rejected validators count; structural review detections use the result limit.
- `max_retained_results`: accepted rule records plus structural review records
  owned by the session. Each consumes one slot before insertion. Normalization
  and suppression do not refund the applied count. It is not a final-finding
  count: two records later reduced to one report entry still require two slots.
- `max_stream_storage_bytes`: checked static maximum logical executor payload,
  admitted before execution allocation. Exact requirement succeeds; smaller
  allowance or unrepresentable arithmetic fails explicitly.

None of these limits caps total source length. Counters are checked; source byte,
line and scalar-column overflow is terminal. Result vectors are session-owned
output storage until the completed report transfers to the caller. Their size is
separate from the executor-storage allowance and bounded by the result count
when configured. Finalization may temporarily own annotated and public records
for the same results; this is output storage, not an unbounded reorder queue.

## Logical storage accounting

The checked requirement combines:

- multi-pattern overlap, boundary scratch and active-prefix payload;
- suffix token retention (zero when no suffix rules exist; otherwise 4096 bytes);
- bounded regex buffers and per-pattern execution metadata;
- validator history, pending snapshots/metadata and assembly scratch;
- coordinate history, anchors, cursor, per-rule monotone lookup cursors and prefix pins;
- per-rule contextual planning metadata;
- a fixed structural detector (21 byte slots, scalar-start coordinates and cursor)
  and the three-byte UTF-8 transport carry.

Existing formulas remain authoritative: regex retention derives from complete
match bounds; projected captures use complete-match reach-back. Pending capacity
is `A + C + 1` per after-context rule. Its proof uses distinct complete-match
starts, full validated callbacks, resolution before insertion, and immediate EOF
validation, rather than assuming distinct projected spans. Before-only rules
need no pending records.

Coordinate lookup keeps one cursor per compiled rule rather than a source-sized
line-start index. Literal and suffix starts increase; projected regex starts
cannot precede the previous complete-match end. Prefix completions use pins.
Interleaved rules therefore advance independently, replaying a callback at most
once per rule. A large unrelated regex bound cannot make dense literal lookups
quadratic. Historical lookups replay only the bounded retained tail.

This is a logical-payload admission contract, **not a total heap-memory cap**.
It excludes spare Vec/String capacity, allocator overhead, fixed container
metadata not explicitly charged above, compiled automata, regex-library caches,
validator workspace, caller input and output records. `reserve_exact` does not
promise exact allocator capacity. Validator workspace sees only candidate or
bounded assembled text for admitted rules; library allocation details remain
outside the logical budget. No new dependencies or custom allocator are used.

## Compatibility inventory

The deterministic test `builtin_portfolio_streaming_compatibility_audit` prints
all unsupported pattern IDs, semantic rejection reasons and individually admitted
rule IDs. Its independent and combined results must agree.

| Portfolio | Rules | Patterns | Bounded patterns | Unbounded unsupported | Finite unsupported |
|---|---:|---:|---:|---:|---:|
| Core | 94 | 55 | 3 | 52 | 0 |
| Personal | 4 | 4 | 0 | 0 | 4 |
| Financial | 3 | 3 | 0 | 0 | 3 |
| Combined | 101 | 62 | 3 | 52 | 7 |

All complete portfolios fail streaming admission. The individually admitted Core
rules are `nats.nkey-private-key`, `nats.nkey-seed`, and `telegram.bot-token`.
Bounded matcher counts alone do not establish semantic compatibility. Do not use
`Scanner::default()` in streaming examples or claim complete portfolio support.
EOF reports are equivalent only for configurations passing every gate.
See the [complete rule-level inventory](streaming-compatibility.md) for IDs and
overlapping matcher/semantic rejection reasons.
