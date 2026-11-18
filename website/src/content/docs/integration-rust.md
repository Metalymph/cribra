---
title: "Rust integration"
description: "Select the published Rust crate configuration and use Core-owned scanning."
order: 10
---

# Rust integration

> **Source and distribution boundary:** This guide describes the repository
> contracts recorded in the W2.2 audit. Source implementation, local tests,
> CI qualification, and publication are distinct claims. Consult the linked
> adapter README and compatibility matrix before shipping binaries.

Cribra performs deterministic scanning in Rust Core. A **finding** is an
authoritative detection; a **sensitive candidate** is review-only and must not
be silently transformed. Spans are zero-based, half-open **UTF-8 byte ranges**.
Streaming admits only compatible rulesets; an ordinary default scanner is not
automatically streamable. Resource budgets are logical limits, not process-wide
heap guarantees.

## Install and select capabilities

Rust Core `cribra` **0.7.2** is published. Its configurations are Cargo feature
*combinations*, not features literally named `core`, `flow`, or `fuse`.

| Configuration | Cargo dependency |
| --- | --- |
| Core | `cribra = { version = "0.7.2", default-features = false }` |
| Flow (default) | `cribra = "0.7.2"` |
| Fuse | `cribra = { version = "0.7.2", default-features = false, features = ["transform", "serde", "parallel"] }` |

Core provides built-ins, custom rules, scanning, reporting, streaming and
incremental sessions. Flow adds transformations and Share Bundles. Fuse adds
serialization and parallel scanning.

## First steps

Build a scanner using `Scanner::builder()`, choose explicit built-in catalogs
or custom rules, then call `scan()` for a complete report. The financial and
personal catalogs are opt-in; custom-only rules do not implicitly enable
built-ins. Use `parallel_scan()` only when the `parallel` feature is enabled.

For exact compilable code, consult [`examples/scan_basic.rs`](https://github.com/Metalymph/cribra/blob/main/examples/scan_basic.rs),
[`examples/custom_rules.rs`](https://github.com/Metalymph/cribra/blob/main/examples/custom_rules.rs), and [`examples/query_results.rs`](https://github.com/Metalymph/cribra/blob/main/examples/query_results.rs) in the checkout.
The API reference covers `ScanReport`, `ScanQuery`, `ScanSummary` and
rule metadata.

## Session selection

Use `stream()` or `stream_with_limits()` for append-only byte input after
ruleset admission. Use `incremental_session()` or
`incremental_session_with_limits()` for revisioned UTF-8 document edits.
Streaming produces a complete report only after successful EOF; incremental
sessions expose committed reports and revisions under their own lifecycle.

## Source references

- [`docs/GETTING_STARTED.md`](https://github.com/Metalymph/cribra/blob/main/docs/GETTING_STARTED.md)
- [`docs/COMPATIBILITY.md`](https://github.com/Metalymph/cribra/blob/main/docs/COMPATIBILITY.md)
- [`examples/scan_basic.rs`](https://github.com/Metalymph/cribra/blob/main/examples/scan_basic.rs)
- [`src/lib.rs`](https://github.com/Metalymph/cribra/blob/main/src/lib.rs)
