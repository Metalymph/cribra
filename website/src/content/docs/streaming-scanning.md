---
title: Streaming scanning
description: Append-only UTF-8 byte sessions, explicit rule admission, budgets, and terminal errors.
order: 17
---

## Contract and use cases

Rust's synchronous streaming API processes **append-only byte fragments** without silently switching to whole-source scanning. `Scanner::stream()` opens a session with unlimited optional limits; `Scanner::stream_with_limits(StreamLimits)` applies caller-supplied limits. A successful consuming `finish(self)` signals genuine EOF and returns a complete `ScanReport`.

Streaming is not an incremental document editor: it cannot revise previously supplied bytes. If edits are required, use [incremental scanning](/docs/incremental-scanning/).

## Admission comes first

Not every valid whole-source scanner is streamable. In particular, `Scanner::default()` and complete current, personal and financial built-in portfolios are **not automatically admitted**. The session rejects unsupported rule configurations instead of silently omitting detections or falling back to whole-source execution. Consult the [current streaming contract](https://github.com/Metalymph/cribra/blob/main/docs/streaming.md) and [rule-level compatibility inventory](https://github.com/Metalymph/cribra/blob/main/docs/streaming-compatibility.md) before choosing a ruleset. Historical broader-admission proposals are not current guarantees.

## Fragment handling and EOF

Byte fragments can split UTF-8 scalar values or CRLF boundaries. The session reconstructs valid UTF-8 transport across fragments before applying matching and coordinate semantics. Empty fragments are no-ops. Genuine EOF matters: an incomplete UTF-8 sequence at `finish()` is an error, not an implicitly repaired input. Do not decode arbitrary fragments independently before passing them to the byte-stream API.

## Optional budgets

The Rust streaming contract describes three independently optional budgets:

| Budget | What it accounts for |
| --- | --- |
| `max_candidates` | Cumulative raw rule emissions, including candidates later rejected by validation or suppression. |
| Logical executor storage | Admitted executor payload according to the static storage model. |
| Retained results | Accepted result records retained during the session, with accounting defined by the Core contract. |

Limits default to unlimited. **Logical storage accounting is not a process-wide heap cap** or a guarantee against allocator failure. Do not equate candidate counts with final finding counts.

## Errors and lifecycle

Session execution failures are terminal: there is no partial report to treat as a successful scan. A failed `push` must not be retried as if the session were still healthy. A completed session is consumed by `finish()`; create a new session for a new source. The [Core streaming contract](https://github.com/Metalymph/cribra/blob/main/docs/streaming.md) defines error precedence, admission and exact budget accounting.

## Other language surfaces

The source checkout exposes documented streaming interfaces through Rust, C ABI, UniFFI bindings and WASM. The CLI does **not** expose a streaming command. For WASM, source implementation does not establish that the independently published `cribra-wasm` distribution includes the same session APIs. See the [integration guides](/docs/integration-wasm/) and [compatibility record](https://github.com/Metalymph/cribra/blob/main/docs/COMPATIBILITY.md).
