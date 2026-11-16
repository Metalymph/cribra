# cribra-mcp

`cribra-mcp` is Cribra's native Rust Model Context Protocol (MCP) server over
stdio. M2 provides functional `cribra_scan` and `cribra_transform` execution
backed directly by the Cribra Core Rust crate. It does not use an async runtime,
an MCP SDK, FFI, a subprocess, HTTP, or an intermediate text serialization for
Core calls.

## Status and scope

Implemented:

- newline-delimited JSON-RPC 2.0 over stdio;
- modern MCP `2026-07-28` stateless requests with per-request `_meta`;
- legacy MCP `2025-11-25` `initialize` / `notifications/initialized`
  lifecycle;
- dual-era coexistence in one process;
- `server/discover`, `tools/list`, and `tools/call` request surfaces;
- exactly two functional tools: `cribra_scan` and `cribra_transform`;
- built-in Core rules plus up to 32 bounded custom regular-expression rules;
- privacy-safe finding and candidate metadata for `cribra_scan`;
- Core-produced redaction and semantic templates for `cribra_transform`;
- manual, source-free argument validation and a fixed tool error taxonomy;
- bounded queues, bounded message sizes, bounded result materialization,
  cancellation lifecycle tracking, prompt EOF shutdown with queued-work
  discard, broken-pipe handling, and explicit terminal backpressure
  termination;
- fixed, source-free protocol errors and diagnostics.

Not implemented:

- keyed pseudonymization or keyed synthesis, and any key material accepted
  through tool arguments;
- resources, prompts, logging, completion, tasks, sampling, elicitation, or
  multi-round tool requests;
- HTTP transport or authorization;
- arbitrary filesystem access;
- server-side storage, caching of scan results, or cross-request state.

## Tool contracts

Exactly two tools are advertised.

### `cribra_scan`

Input: `source` plus optional `rules`.

Output `structuredContent`:

```json
{
  "status": "ok",
  "findings": [
    {
      "ruleId": "github.classic-pat",
      "start": 7,
      "end": 47,
      "line": 1,
      "column": 8,
      "severity": "critical",
      "confidence": "high",
      "remediation": "revoke_and_rotate_credential"
    }
  ],
  "candidates": [],
  "summary": {
    "scannedSources": 1,
    "scannedBytes": 54,
    "reportsWithFindings": 1,
    "reportsWithoutFindings": 0,
    "totalFindings": 1,
    "totalCandidates": 0,
    "reportsWithCandidates": 0,
    "critical": 1,
    "high": 0,
    "medium": 0,
    "low": 0,
    "info": 0
  }
}
```

`start` and `end` are zero-based UTF-8 byte offsets. `line` and `column` are
one-based; columns count Unicode scalar values. Findings preserve Core ordering,
severity, confidence, remediation, and locations. Review candidates expose only
`kind`, `evidence`, and location metadata.

`cribra_scan` never returns the source, a matched substring, surrounding
context, or candidate text. The first text content block is a counter sentence
such as `Scan complete: 2 findings, 1 candidate.` It contains no source text.

### `cribra_transform`

Input: `source`, `mode` (`redact` or `template`), optional `rules`, and
mode-specific `options`.

- `redact`: replaces all findings with `[REDACTED]` by default, or with the
  validated `options.replacement`.
- `template`: replaces findings with semantic placeholders such as
  `<CRIBRA:github.classic-pat>`. `options.namespace` defaults to `CRIBRA`;
  `options.numbered` defaults to `false` and adds per-rule occurrence numbers.

Output `structuredContent`:

```json
{
  "status": "ok",
  "mode": "redact",
  "output": "token [REDACTED]",
  "bytesIn": 21,
  "bytesOut": 15,
  "findingCount": 1
}
```

The same Core-produced string is returned as `structuredContent.output` and as
the first text content block. Unchanged source regions remain visible in the
output. `findingCount` is the number of Core findings, not the number of merged
redaction markers. When findings overlap, redaction merges their spans; template
generation returns an `overlapping_spans` tool error.

`cribra_transform` is not anonymization and does not claim that all sensitive
data was detected. Candidates and undetected sensitive values may remain.

## Input validation and limits

The published JSON schemas under `schemas/` are the client-facing contracts.
Runtime validation additionally enforces UTF-8 byte limits and Core constraints
that a JSON schema alone cannot express.

| Limit | Value |
| --- | --- |
| `source` | 1 MiB UTF-8 bytes |
| custom rules per call | 32 |
| custom rule `id` | 1–128 ASCII characters, `[A-Za-z0-9._-]` |
| custom rule `pattern` | 1–4096 Unicode scalar values |
| findings returned | 4096 |
| candidates returned | 4096 |
| redaction replacement | 1–256 Unicode scalar values |
| template namespace | 1–64 ASCII characters, `[A-Za-z0-9._-]` |
| request line | 2 MiB, excluding newline |
| encoded response | 8 MiB, including the JSON-RPC envelope and newline |
| worker request queue | 8 |
| writer response queue | 8 |
| cancellation registry | 1024 identifiers |
| request id string | 128 UTF-8 bytes |

Custom rule `id` values must be unique and must not collide with built-in rule
identifiers. Invalid regular expressions and patterns that can match empty input
are rejected. Custom rules use matcher-authoritative semantics: they add
detection patterns but cannot select built-in validators, remediation, or
transformation behavior. Built-in rules are always included.

More than 4096 findings or 4096 candidates is a hard `result_limit_exceeded`
error. Results are never truncated.

## Execution and resource behavior

The worker owns all Core calls. The reader thread never performs scanning or
transformation. Core execution is synchronous and is not preemptible; there is
no safe cooperative cancellation API for a whole-source scan.

### Bounded result materialization

MCP uses Core's public `IncrementalLimits` API with a combined materialized
result budget of `4096 + 65536 = 69632`. The second term is a conservative
upper bound on structural candidates: a recovery-like candidate spans 19 bytes,
and consecutive candidates must start at least 20 bytes apart, so a 1 MiB source
cannot produce more than about 52,429 of them.

Core's materialization counter includes structural candidates *before* the
candidates that overlap findings are suppressed. The finalized report can
therefore have fewer candidates than the counter. A budget of exactly
`4096 + 4096` would reject a valid scan with many suppressed structural
candidates and no surviving candidate. The 65536 term keeps the budget above
every report that satisfies the per-channel finding and candidate limits, while
still bounding pathological finding materialization.

The remaining residual risk is raw candidate emission. A pathological custom
pattern such as `(?s).` can emit one raw candidate per source byte before
normalization. Source input is capped at 1 MiB, but the raw candidate cache is
still proportional to the input. Measured peak memory for that case is
approximately 183 MB; the MCP layer rejects the result with
`result_limit_exceeded` after Core stops at the materialization budget. There is
no public Core API that preserves exact finding semantics and terminates at the
first candidate beyond both 4096-channel limits, so Core is not modified merely
to optimize MCP.

Transformations only consume findings, so the larger budget also ensures that
structural candidates cannot cause a transform rejection; the finding limit
still bounds pathological finding materialization.

### Scanner construction and caching

- Calls without custom rules use `Scanner::default()`, which shares Core's
  process-wide compiled built-in catalog.
- Calls with custom rules build a new scanner from the built-in catalog plus the
  validated custom rules. This recompiles the built-in catalog for that call.
  No unbounded scanner cache is added.
- A source string is moved into Core's incremental session. This avoids an
  additional unbounded copy; the only ownership cost is the bounded 1 MiB
  source itself.

### Response-size correctness

Before a tool result is emitted, the worker serializes the already era-shaped
result with a counting writer, serializes the request identifier, and adds the
fixed JSON-RPC success envelope. The check covers `resultType`, modern `_meta`,
request id escaping, all JSON string escaping, both content representations,
and the trailing newline. If the exact encoded response would exceed 8 MiB, the
worker replaces the result with the fixed `response_too_large` tool error.

This is an exact byte count, not an estimate, so no unproven envelope margin is
used.

## Error model

Malformed envelopes, unknown methods, unknown tools, non-object `arguments`,
duplicate live request identifiers, and unsupported protocol revisions remain
JSON-RPC errors. A syntactically valid tool call with an execution-relevant
problem returns an MCP tool result with `isError: true` and a schema-conformant
`structuredContent.error` object.

| Code | Condition |
| --- | --- |
| `invalid_arguments` | shape, type, unknown field, length, identifier, or enum violation |
| `invalid_rule_configuration` | duplicate/colliding rule id or scanner build failure |
| `invalid_rule_pattern` | invalid regular expression or a pattern that can match empty input |
| `unsupported_mode` | transform mode other than `redact` or `template` |
| `overlapping_spans` | template transformation received overlapping findings |
| `result_limit_exceeded` | more than 4096 findings or candidates, or the combined materialization budget was exceeded |
| `response_too_large` | the complete encoded JSON-RPC response would exceed 8 MiB |
| `internal_error` | unexpected Core, projection, or internal failure |

Every message is a fixed static string of at most 256 bytes. Messages never
interpolate source text, matched values, custom patterns, rule identifiers, Core
error text, options, or key material. Core error text is mapped, not forwarded.
Custom rule identifiers are client-supplied metadata; clients must not encode
secrets in them.

Panics in the worker are caught and converted to the same bounded
`internal_error` tool result rather than a protocol-level `-32603` response.

## Protocol behavior

### Advertised versions

The modern `server/discover` result advertises exactly:

```json
{
  "supportedVersions": ["2026-07-28"],
  "capabilities": { "tools": {} },
  "ttlMs": 0,
  "cacheScope": "private"
}
```

No other modern revision is advertised. Legacy `initialize` negotiates
`2025-11-25` and returns that version in the `initialize` result.

### Modern `2026-07-28`

Every modern request must carry both `_meta` members:

- `io.modelcontextprotocol/protocolVersion`
- `io.modelcontextprotocol/clientCapabilities`

Missing or malformed `_meta` returns `-32602` with the fixed message
`Missing required _meta fields`. An unsupported version returns `-32022` with
`data.supported = ["2026-07-28"]` and a bounded, sanitized `data.requested`.
Modern results carry `resultType: "complete"` and a server-info `_meta` entry.
`server/discover` and `tools/list` return `ttlMs: 0` and
`cacheScope: "private"`.

Modern `initialize` and `ping` are not methods in this revision and return
`-32601`. A modern `server/discover` request does not commit the process to
modern-only operation.

### Legacy `2025-11-25`

A request whose method is `initialize` and whose params do not contain the
modern protocol-version metadata starts the legacy lifecycle. A successful
`initialize` moves the session to `AwaitingInitialized`; the
`notifications/initialized` notification moves it to `Active`. A second
`initialize` returns `-32600` with `Initialize already completed`. A request
with neither modern `_meta` nor any legacy `initialize` returns `-32602` with
`Missing required _meta fields`. After a successful `initialize` but before
`notifications/initialized`, non-ping requests return `-32600` with
`Server not initialized`; legacy pings remain available. Legacy `ping` returns
an empty result. Legacy results do not include `resultType`, `ttlMs`, or
`cacheScope`.

A modern request and a legacy lifecycle may coexist in the same process. The
per-request `_meta` decides how that request is shaped.

Protocol error messages are fixed. Request source text, tool arguments,
patterns, method names, and key material are never interpolated into protocol
errors or stderr diagnostics. When a request identifier cannot be read, the
error response omits `id`, matching the pinned `2026-07-28` schema.

## Transport architecture

Three roles run concurrently:

1. **Reader** owns stdin, the bounded line decoder, era classification, legacy
   lifecycle transitions, inline `notifications/cancelled` handling, and
   acceptance of requests into the bounded worker queue.
2. **Worker** validates arguments, calls Cribra Core, projects findings and
   transformations, checks the exact response size, and submits responses.
3. **Writer** (the main thread) owns stdout and the final cancellation claim at
   the emission boundary.

The reader never performs a blocking stdout write. stdout is switched to
nonblocking; writes use `poll` and reset a no-progress deadline whenever any
byte is accepted. The default no-progress deadline is 5000 ms. Sustained
no-progress writing is terminal: the process prints one fixed stderr diagnostic
and exits with code `3`. The deadline can be shortened for host tests with
`CRIBRA_MCP_BACKPRESSURE_TIMEOUT_MS` (clamped to 10..600000 ms). This override is
not a client-facing protocol option.

`SIGPIPE` is ignored so a closed stdout becomes a deterministic `EPIPE` result.
A broken stdout pipe exits cleanly with code `0`; it does not hang waiting for
stdin.

Closing stdin stops further acceptance immediately. The worker finishes at most
the tool call that is already executing, discards every not-yet-started queued
call, and releases their request identifiers. Because Core calls are
synchronous, shutdown latency can include one in-progress scan or transform,
bounded by the 1 MiB source limit. The writer finishes at most one response it
had already dequeued or a JSON-RPC line already in progress, discards the
remaining queued responses, releases their identifiers, and exits `0`. Queued
source buffers are dropped with their queue entries.

On normal input closure the process joins the reader and worker so the
already-running call can finish and release its state. Terminal backpressure,
broken pipe, and internal failures return immediately without joining.

### macOS and Linux pipe behavior

The transport relies only on POSIX `O_NONBLOCK`, `poll`, `write`, `EPIPE`, and
`SIGPIPE`; it does not assume a particular pipe capacity. Pipe writes larger
than `PIPE_BUF` may be partial, so every short write is counted and the
remaining bytes are retried. A full pipe returns `EAGAIN`/`EWOULDBLOCK`; the
writer polls for writability and treats a completed write of any length as
progress.

Tests do not depend on an exact pipe capacity: the in-process test fills a pipe
until writes would block, and the process test queues twelve roughly 8 KiB
`tools/list` responses without reading stdout. A closed read end yields `EPIPE`
rather than an asynchronous `SIGPIPE` termination.

### Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Normal EOF shutdown after discarding queued work, or the peer closed stdout (`EPIPE`) |
| `1` | Internal reader/encoder/registry/write failure |
| `2` | Reserved for startup/configuration failure |
| `3` | Sustained stdout backpressure or an exhausted output queue |

## Cancellation and identifier lifetime

`notifications/cancelled` is handled inline by the reader. A request identifier
is tracked from acceptance until its response is fully written, its queued
response is skipped because of cancellation, or it is discarded by shutdown.

Registry states are `Pending`, `Processing`, `Emitting`, and `Cancelled`:

- cancellation before the worker starts skips execution and releases the
  identifier because no result can exist;
- cancellation during a Core call or after the result is queued is claimed by
  the writer before emission, and the queued response is skipped;
- cancellation after the writer has claimed the emission boundary is ignored,
  because a partially written JSON-RPC line cannot be retracted;
- stale, duplicate, unknown, and malformed cancellations produce no response
  and do not affect unrelated requests.

A duplicate live identifier is rejected with `-32600` while the original
request remains tracked. An identifier becomes reusable only after a completed
write, a discarding shutdown, or a pre-execution cancellation that produced no
result. A cancellation that arrives after an identifier has been fully released
is ignored, so it cannot poison a later reuse.

Core execution is not made preemptible and is not presented as preemptible. A
cancelled call may still consume worker time until Core returns; its result is
discarded at the emission boundary.

## Privacy guarantees and limitations

- `cribra_scan` returns metadata only. It does not return source text, matched
  substrings, context, candidate values, or request arguments.
- Error results and stderr diagnostics use fixed strings and do not echo source
  text, patterns, matched values, rule identifiers, or Core error text.
- `cribra_transform` intentionally returns transformed source because that is
  its purpose. Unchanged regions remain visible. It is not anonymization, and
  undetected sensitive data or candidates may remain.
- Custom rule patterns are client-supplied and are compiled in memory. They are
  never returned in results or errors. Clients should not put secrets in rule
  identifiers or patterns.
- No key material is accepted. Keyed pseudonymization and synthesis are not
  advertised and cannot be selected through MCP.

## Official schema and conformance evidence

The official MCP JSON schemas are vendored under `tests/schemas/` from the
`2026-07-28` and `2025-11-25` release tags of
`modelcontextprotocol/modelcontextprotocol` and pinned by SHA-256:

| Revision | Schema SHA-256 |
| --- | --- |
| `2026-07-28` | `ef70b61f99b6d2e5e3b46863822eab08dff6a45bedc7a08914e0e5b133f40203` |
| `2025-11-25` | `1ffe4c5577974012f5fa02af14ea88df4b7146679df1abaaad497c8d9230ca8a` |

The suite validates every pinned fixture request and response against the
official schema for its wire era, checks the hashes, runs real-binary
transcripts for modern, legacy, and dual-era operation, and validates every
successful and failed tool `structuredContent` against the advertised tool
schemas.

The official `modelcontextprotocol/conformance` server runner at the time of M2
is URL/HTTP-SSE based (`--url`) and does not launch a stdio server. Its
`2026-07-28` requirement set also covers resources, prompts, logging, caching,
and multi-round tool scenarios beyond this crate's declared scope. This crate
does **not** claim an official conformance-suite pass or certification.

M3 adds a reproducible qualification record:
[`CONFORMANCE.md`](CONFORMANCE.md) holds the generated requirement matrix for
both pinned revisions (147 canonical rows, each classified as applicable,
partial or not applicable with its prerequisite spelled out), the evidence for
every passing row, and an explicit PASS/FAIL/PARTIAL verdict.

Independent SDK clients are pinned and run by
`crates/cribra-mcp/qualification/run-qualification.sh`:

- TypeScript `@modelcontextprotocol/client` 2.4.0 — 5 scenarios covering pinned
  modern negotiation, `auto` negotiation, the legacy handshake, an unexpected
  server exit, and clean close;
- Python `mcp` 2.3.0 — 5 scenarios covering a pinned modern client (which
  performs **no** discovery probe; asserted rather than assumed), explicit
  `server/discover` over the wire, `auto` negotiation, legacy handshake with the
  tool-error taxonomy, and clean close.

Each harness re-validates tool results with a third-party JSON Schema validator
against the advertised output schemas. The legacy *fallback* negotiation path is
not exercised against this server, because the server answers `server/discover`,
and is not claimed.

## Measurements

Measurements are local, non-normative, and may vary by host and filesystem
cache state. They were taken with a release build on macOS arm64.

| Measurement | Result |
| --- | --- |
| Stripped binary | 2,404,448 bytes |
| Resolved normal crates | 23, including both workspace crates |
| Startup median / p95 (`server/discover`) | 2.95 ms / 4.60 ms (n = 80) |
| First built-in scan, 1 MiB clean source | ~233 ms, ~125 MB peak RSS |
| First built-in scan, 1 MiB source with one finding | ~242 ms, ~125 MB peak RSS |
| Built-in-only scan, 4 KiB source | ~179 ms, ~124 MB peak RSS |
| Custom-rule scan, 4 KiB source, one rule | ~187 ms, ~126 MB peak RSS |
| Custom-rule scan, 4 KiB source, 32 rules | ~195 ms fresh, ~190 ms steady-state |
| Pathological `(?s).`, 1 MiB source | ~270 ms, ~183 MB peak RSS, `result_limit_exceeded` |

Core's built-in scanner compilation dominates the first-call result: the same
binary's startup is about 3 ms, while the first scan builds the cached built-in
catalog and executes it. Steady-state built-in scans in a warm process are about
0.1 ms for a 4 KiB source. Custom-rule calls rebuild the built-in catalog on
every call because Core exposes no public merge-with-builtins API; this is
bounded but measured at roughly 175-190 ms per call on this host.

## Qualification

From the workspace root:

```sh
cargo fmt --all -- --check
cargo clippy -p cribra-mcp --all-targets -- -D warnings
cargo clippy -p cribra-mcp --all-targets -- -D warnings # with +1.97.0
cargo test -p cribra-mcp --all-targets
cargo test -p cribra-mcp --all-targets # with +1.97.0
cargo test -p cribra
cargo build --release -p cribra-mcp

# M3 conformance matrix and independent SDK qualification
just mcp-matrix
just mcp-qualify
```

Expected test coverage:

- JSON-RPC parsing, request IDs, oversized lines, and bounded decoding;
- modern discovery, `_meta` validation, version errors, and result shaping;
- legacy initialize/initialized lifecycle, ping, and second-initialize error;
- dual-era coexistence without discovery committing the session;
- exactly two tool contracts and their schema accept/reject cases;
- argument validation for every limit, type, field, mode, and rule shape;
- built-in and custom-rule scan execution, rule identity, severity,
  confidence, remediation, byte offsets, Unicode columns, and multiline
  positions;
- candidate metadata without candidate values;
- custom rule compilation errors, duplicate IDs, and built-in collisions;
- hard finding/candidate limits with no truncation;
- suppressed structural candidates not causing a false limit error;
- redact and template execution, defaults, namespaces, numbering, overlap
  behavior, Unicode preservation, and exact byte counters;
- exact response-size accounting including modern metadata, request IDs, and
  JSON escaping;
- source-free scan results and sanitized success/error results;
- schema validation of every success and failure tool result;
- modern, legacy, and dual-era real-binary JSONL transcripts;
- cancellation before execution, during execution, after result enqueue, and
  at the emission boundary;
- stdout full-pipe timeout with exit `3`;
- broken stdout pipe shutdown;
- EOF with queued real scan work, and cancellation during real execution;
- stdout cleanliness, source-free errors, and normal EOF exit;
- adversarial framing: empty lines, batch arrays, CRLF delimiters, invalid
  UTF-8, recursion-limited nesting, maximum-length identifiers, oversized
  frames and duplicate live identifiers;
- 1 MiB source-size boundary exactly at and one byte over the limit;
- canary-based privacy checks for scan results, tool errors, stderr and
  transform text/structured agreement;
- partial pipe writes for a multi-chunk response, and a mid-response client
  disconnect;
- the generated conformance matrix itself: completeness, uniqueness, verbatim
  canonical classification, and existence of every cited test.

## M2 exit criteria

M2 is complete when all of the following are true:

1. Both tools execute against Core directly with no stub path.
2. Built-in and bounded custom-regex scans return accurate, source-free
   finding and candidate metadata.
3. Redact and template modes return Core-produced output with accurate
   `bytesIn`, `bytesOut`, and `findingCount`.
4. More than 4096 findings or candidates is a hard error, never truncation.
5. The complete encoded response, including protocol-era metadata, request ID,
   and JSON escaping, is checked exactly against the 8 MiB limit.
6. Argument, rule, mode, overlap, result-limit, response-size, and internal
   failures map to fixed sanitized tool errors; protocol failures remain
   JSON-RPC errors.
7. Keyed modes are not advertised and no key material is accepted.
8. Modern, legacy, and dual-era behavior, cancellation, EOF, backpressure,
   broken-pipe, and request-registry guarantees remain covered by deterministic
   tests.
9. All prescribed Rust 1.97.0 and current-toolchain format, Clippy, and test
   gates pass.
10. README, schemas, and tests contain no M1 stub claims.

## Platform support

M2 supports macOS and Linux stdio transports. Non-Unix targets fail at compile
time. Windows stdio behavior is not claimed.
