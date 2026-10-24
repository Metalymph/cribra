---
title: Scanning modes
description: Choose whole-source scanning, admitted append-only byte streams, or revisioned incremental documents.
order: 4
---

## Whole-source scanning

`Scanner::scan()` accepts caller-supplied sources and returns ordered complete reports. It is the semantic reference for the other modes and supports built-in portfolios. Optional parallel scanning processes independent sources through Rayon while preserving deterministic input order.

## Streaming

Streaming accepts byte fragments through a synchronous session and produces a complete `ScanReport` only when successful consuming `finish()` signals genuine EOF. UTF-8 scalars may span transport fragments; the engine reconstructs them before matching and coordinate accounting.

Admission is explicit. The default scanner and complete current, personal, and financial portfolios are not automatically streamable. Unsupported configurations fail before execution rather than silently falling back to partial detection.

Three optional limits govern raw candidates, logical executor storage, and retained results. Limits are unlimited by default; logical storage accounting is not a total heap-memory cap. Execution failures are terminal and return no partial report.

Read the [streaming contract](https://github.com/Metalymph/cribra/blob/main/docs/streaming.md) and [compatibility inventory](https://github.com/Metalymph/cribra/blob/main/docs/streaming-compatibility.md) before selecting a ruleset.

## Incremental scanning

`Scanner::incremental_session(String)` owns a complete revisioned UTF-8 document. Edits supply an expected revision, a byte range on UTF-8 scalar boundaries, and replacement text. Successful transactions expose a complete report matching a fresh whole-source scan.

Incremental scanning supports the built-in portfolios independently of streaming admission. Literal and structural matching can localize work; prefix, suffix, and regex rules use exact whole-rule replay. Incremental is not append-only streaming.

Invalid edits such as stale revisions or invalid boundaries are recoverable. Resource or invariant failures terminate further edits while preserving the previous complete committed snapshot. Four optional limits cover raw emissions, peak cache ownership, result materialization, and logical transaction payload. The session owns the full document; it does not promise bounded document retention or a total heap cap.

See the [incremental contract](https://github.com/Metalymph/cribra/blob/main/docs/incremental.md) for failure precedence and accounting details.

## Integration coverage

The checkout's Rust, C ABI, WASM, Swift, Python, and Kotlin/JVM surfaces expose their documented session APIs. The CLI does not expose streaming or incremental-document commands. Checkout availability does not establish published-package availability; compare the [bindings matrix](/bindings/).
