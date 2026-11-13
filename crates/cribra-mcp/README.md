# cribra-mcp

`cribra-mcp` is Cribra's native Rust Model Context Protocol (MCP) server
foundation over stdio. **M1 is a protocol and transport foundation, not a
functional scan or transform server.** The two advertised tools are static
contracts with deliberate stub handlers.

This crate does not use an async runtime or an MCP SDK. It implements the wire
format directly with `serde`/`serde_json`, `libc` for bounded stdio, and
`std` threads.

## Status and scope

Implemented in M1:

- newline-delimited JSON-RPC 2.0 over stdio;
- modern MCP `2026-07-28` stateless requests with per-request `_meta`;
- legacy MCP `2025-11-25` `initialize` / `notifications/initialized`
  lifecycle;
- dual-era coexistence in one process;
- `server/discover`, `tools/list`, and `tools/call` request surfaces;
- exactly two static tool contracts: `cribra_scan` and `cribra_transform`;
- bounded queues, bounded message sizes, cancellation lifecycle tracking,
  prompt EOF shutdown with queued-work discard, broken-pipe handling, and
  explicit terminal backpressure termination;
- fixed, source-free protocol errors and diagnostics.

Not implemented in M1:

- real `cribra_scan` or `cribra_transform` execution;
- resources, prompts, logging, completion, tasks, sampling, elicitation, or
  multi-round tool requests;
- HTTP transport or authorization;
- arbitrary filesystem access;
- keyed transform modes or process-owned key material.

**Do not treat a successful M1 tool call as functional scanning or
transformation interoperability.** Every tool call currently returns
`isError: true`.

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
`2025-11-25` and returns that version in the `initialize` result. These are the
only two wire versions implemented and tested by M1.

### Modern `2026-07-28`

Every modern request must carry both `_meta` members:

- `io.modelcontextprotocol/protocolVersion`
- `io.modelcontextprotocol/clientCapabilities`

Missing or malformed `_meta` returns `-32602` with the fixed message
`Missing required _meta fields`. An unsupported version returns `-32022` with
`data.supported = ["2026-07-28"]` and a bounded, sanitized
`data.requested`. Modern results carry `resultType: "complete"` and a
server-info `_meta` entry. `server/discover` and `tools/list` return
`ttlMs: 0` and `cacheScope: "private"`.

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

### Errors

Protocol errors use fixed messages. Request source text, tool arguments,
patterns, method names, and key material are never interpolated into errors or
stderr diagnostics. When a request identifier cannot be read, the error
response omits `id`, matching the pinned `2026-07-28` schema and specification.
An unsupported requested version is sanitized and bounded before inclusion.

## Tool contracts

Exactly two tools are advertised:

| Tool | M1 contract | M2 intent |
| --- | --- | --- |
| `cribra_scan` | Input source plus optional custom rules; output privacy-safe finding and candidate metadata. | Return finding metadata only; never source fragments. |
| `cribra_transform` | Input source, `mode: redact` or `mode: template`, and mode-specific options. | Return Core-produced transformed source in `structuredContent.output` and as a compatible first text block. Unchanged source parts stay visible. |

`cribra_scan` is read-only and should not echo the source. `cribra_transform`
is a share-safe transformation, not anonymization; unchanged content remains
visible. Keyed pseudonymization and keyed synthesis are intentionally deferred
to M2, must be disabled by default, and must use process-owned key material
rather than accepting key material in tool arguments.

### Custom-rule decision

M1 deliberately exposes a bounded, regex-only custom-rule shape in both static
contracts:

- at most 32 rules;
- `id` (bounded identifier), `pattern` (bounded string), and `severity`;
- no private built-in validator selection, no output, remediation, or
  transformation semantics on a custom rule.

This mirrors Core's matcher-authoritative custom-rule model while keeping MCP
input small and inspectable. The M1 stub never compiles or executes a pattern.
Broader custom-rule families and runtime admission rules remain M2 work.

### M1 stub result

Both handlers return the same fixed result shape:

```json
{
  "content": [
    { "type": "text", "text": "Tool execution is unavailable in this M1 build" }
  ],
  "isError": true,
  "structuredContent": {
    "status": "error",
    "error": {
      "code": "internal_error",
      "message": "Tool execution is unavailable in this M1 build"
    }
  }
}
```

Tool input and output schemas are embedded from `schemas/` and advertised via
`inputSchema` / `outputSchema`. They are contracts for M2 and are validated in
the test suite; they do not imply working handlers.

## Transport architecture

Three roles run concurrently:

1. **Reader** owns stdin, the bounded line decoder, era classification, legacy
   lifecycle transitions, inline `notifications/cancelled` handling, and
   acceptance of requests into the bounded worker queue.
2. **Worker** owns tool execution. M1 executes only the fixed stub.
3. **Writer** (the main thread) owns stdout and the final cancellation claim at
   the emission boundary.

Bounds:

- request line: 2 MiB;
- serialized response: 8 MiB;
- worker queue: 8;
- writer queue: 8;
- cancellation registry: 1024 identifiers;
- request id string: 128 UTF-8 bytes.

The reader never performs a blocking stdout write. stdout is switched to
nonblocking; writes use `poll` and reset a no-progress deadline whenever any
byte is accepted. The default no-progress deadline is 5000 ms. Sustained
no-progress writing is terminal: the process prints one fixed stderr
diagnostic and exits with code `3`. The deadline can be shortened for host
tests with `CRIBRA_MCP_BACKPRESSURE_TIMEOUT_MS` (clamped to 10..600000 ms).
This override is not a client-facing protocol option.

`SIGPIPE` is ignored so a closed stdout becomes a deterministic `EPIPE`
result. A broken stdout pipe exits cleanly with code `0`; it does not hang
waiting for stdin.

Closing stdin stops further acceptance immediately. The worker finishes at
most the tool call that is already executing, discards every not-yet-started
queued call, and releases their request identifiers. The writer finishes at
most one response it had already dequeued or a JSON-RPC line already in
progress, discards the remaining queued responses, releases their identifiers,
and exits `0`. Shutdown latency is therefore bounded by one in-progress
execution and one in-progress write, not by the accumulated backlog. Queued
source buffers are dropped with their queue entries.

Because the writer will time out rather than block forever, stdin EOF cannot
mask a permanently blocked stdout write. If stdout is already blocked when
stdin closes, the in-progress write still terminates through the configured
no-progress deadline with exit `3`.

On normal input closure the process joins the reader and worker so the
already-running call can finish and release its state. Terminal backpressure,
broken pipe, and internal failures return immediately without joining, because
the connection is already unusable and the process is about to exit.

### macOS and Linux pipe behavior

The transport relies only on POSIX `O_NONBLOCK`, `poll`, `write`, `EPIPE`, and
`SIGPIPE`; it does not assume a particular pipe capacity. Pipe writes larger
than `PIPE_BUF` may be partial, so every short write is counted and the
remaining bytes are retried. A full pipe returns `EAGAIN`/`EWOULDBLOCK`; the
writer polls for writability and treats a completed write of any length as
progress.

The common macOS and Linux default pipe capacity is 64 KiB, but this is not
guaranteed and can be changed by the host. Tests therefore do not depend on an
exact capacity: the in-process test fills a pipe until writes would block, and
the process test queues twelve roughly 8 KiB `tools/list` responses (more than
the conventional buffer) without reading stdout. A closed read end yields
`EPIPE` rather than an asynchronous `SIGPIPE` termination. Because stdin
reading and stdout writing are in different threads, stdin EOF cannot unblock
or mask a stalled stdout write; only the writer's no-progress deadline ends
that terminal condition.

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
This prevents reuse while a response is pending in either queue.

Registry states are `Pending`, `Processing`, `Emitting`, and `Cancelled`:

- cancellation before the worker starts skips execution and releases the
  identifier because no result can exist;
- cancellation during execution or after the result is queued is claimed by
  the writer before emission, and the queued response is skipped;
- cancellation after the writer has claimed the emission boundary is ignored,
  because a partially written JSON-RPC line cannot be retracted;
- stale, duplicate, unknown, and malformed cancellations produce no response
  and do not affect unrelated requests.

A duplicate live identifier is rejected with `-32600` while the original
request remains tracked. An identifier becomes reusable only after a completed
write, a discarding shutdown, or a pre-execution cancellation that produced no
result.

A cancellation that arrives after an identifier has been fully released is
ignored, so it cannot poison a later reuse. A cancellation that arrives only
after the client has already reused the identifier is indistinguishable from a
cancellation of the new request because the notification carries no generation
or timestamp. Clients must not reuse an identifier while a cancellation for the
old request may still be in flight.

## Official schema and conformance evidence

The official MCP JSON schemas are vendored under `tests/schemas/` from the
`2026-07-28` and `2025-11-25` release tags of
`modelcontextprotocol/modelcontextprotocol` and pinned by SHA-256:

| Revision | Schema SHA-256 |
| --- | --- |
| `2026-07-28` | `ef70b61f99b6d2e5e3b46863822eab08dff6a45bedc7a08914e0e5b133f40203` |
| `2025-11-25` | `1ffe4c5577974012f5fa02af14ea88df4b7146679df1abaaad497c8d9230ca8a` |

The suite validates every pinned fixture request and response against the
official schema for its wire era, checks the hashes, and runs real-binary
transcripts for modern, legacy, and dual-era operation.

The official `modelcontextprotocol/conformance` server runner at the time of
M1 is URL/HTTP-SSE based (`--url`) and does not launch a stdio server. Its
`2026-07-28` requirement set also covers resources, prompts, logging, caching,
and multi-round tool scenarios beyond M1's declared scope. M1 therefore does
**not** claim an official conformance-suite pass or certification.

Version and message compatibility are supported by the pinned official
schemas, pinned transcripts, specification-defined error shapes, and
independent official TypeScript SDK 2.4.0 clients connected directly to the
real binary over stdio:

- modern client with pinned version negotiation: negotiated `2026-07-28`,
  observed `supportedVersions: ["2026-07-28"]`, listed exactly
  `cribra_scan` and `cribra_transform`, and received the fixed `isError` stub
  result;
- legacy client: negotiated `2025-11-25`, reported the legacy era, and listed
  the same two tools.

## Architecture evidence

M1 was chosen after comparing a direct `serde` implementation with `rmcp`
3.9.0 in a local macOS arm64 spike. The direct approach was retained to control
dual-era shaping, cancellation at the writer boundary, diagnostic content, and
bounded stdio behavior.

| Measurement | Direct M1 build | `rmcp` 3.9.0 spike |
| --- | --- | --- |
| Stripped binary | 598,304 bytes | 1,066,720 bytes |
| Unique dependency crates (normal) | 14 | 71 |
| Startup median / p95 | 2.38 ms / 2.75 ms | 3.0 ms / 6.6 ms |
| Max RSS | 2.1 MB | 3.0 MB |
| Advertised discovery versions | only implemented revisions | five revisions, including untested ones |

Measurements are local, non-normative, and may vary by host. The startup sample
contained a scheduler outlier; the median is the useful comparison.

## Qualification

From the workspace root:

```sh
cargo fmt --all -- --check
cargo clippy -p cribra-mcp --all-targets -- -D warnings
cargo test -p cribra-mcp --all-targets
cargo +1.97.0 test -p cribra-mcp --all-targets
cargo +1.97.0 clippy -p cribra-mcp --all-targets -- -D warnings
```

Expected test coverage:

- JSON-RPC parsing, request IDs, oversized lines, and bounded decoding;
- modern discovery, `_meta` validation, version errors, and result shaping;
- legacy initialize/initialized lifecycle, ping, and second-initialize error;
- dual-era coexistence without discovery committing the session;
- exactly two tool contracts and their schema accept/reject cases;
- modern, legacy, and dual-era real-binary JSONL transcripts;
- stdout cleanliness, source-free errors, and normal EOF exit;
- normal operation with one response per accepted request and no stranded
  request;
- cancellation before execution, during execution, after result enqueue, and
  at the emission boundary;
- stdout full-pipe timeout with exit `3`;
- broken stdout pipe shutdown;
- EOF with a blocked worker and a full request queue: only the running call
  executes, queued calls and responses are discarded, and all identifiers are
  released;
- EOF with queued responses: no malformed trailing output and exit `0`.

## M1 exit criteria

M1 is complete when all of the following are true:

1. `cribra-mcp` is the only new workspace crate and the only root workspace
   change is its member registration.
2. `cargo fmt --all -- --check` passes.
3. Clippy passes with `-D warnings` on the current toolchain and Rust 1.97.
4. All unit and JSONL integration tests pass on Rust 1.97.
5. Modern `2026-07-28`, legacy `2025-11-25`, and dual-era fixtures match their
   pinned expectations and validate against the pinned official schemas.
6. Discovery advertises exactly `2026-07-28`; legacy negotiation returns
   exactly `2025-11-25`.
7. Exactly two tools are advertised and every M1 call returns the fixed
   `isError: true` stub result.
8. Cancellation is effective before execution, during execution, after result
   enqueue, and at the writer emission boundary.
9. Request IDs cannot be reused while a response remains pending.
10. Full-pipe backpressure, broken pipe, stdin EOF, queued-work discard, and
    writer shutdown have deterministic tests.
11. stdout contains only JSON-RPC lines, successful operation keeps stderr
    empty, and diagnostics never contain request source or arguments.
12. Rust 1.97 compatibility is verified.
13. No scan or transform interoperability is claimed, and keyed transform
    modes remain disabled and deferred to M2.

## Platform support

M1 supports macOS and Linux stdio transports. Non-Unix targets fail at compile
time. Windows stdio behavior is not claimed.
