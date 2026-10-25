---
title: Rust API reference
description: Navigate Cribra's public Rust API and its documented contracts.
order: 5
---

## API overview

Cribra exposes a deterministic Rust scanning engine. Applications supply sources and control acquisition, persistence, networking, presentation, and policy.

The public API is organized around scanner construction, rules, reports, queries, execution sessions, and explicit transformations.

| Area | Main APIs | Guide |
| --- | --- | --- |
| Scanner configuration | `Scanner`, `ScannerBuilder`, `ScannerBuildError` | [Scanner and configuration](/docs/scanner-configuration/) |
| Rules and catalogs | `Rule`, `RuleSpec`, `RuleKind`, `RuleId` | [Rules and built-ins](/docs/rules-builtins/) |
| Classification | `Finding`, `SensitiveCandidate`, `Explanation` | [Findings and reports](/docs/findings-reports/) |
| Reporting | `ScanReport`, `ScanResults`, `ScanEntry` | [Findings and reports](/docs/findings-reports/) |
| Queries | `ScanQuery`, `SortedScanQuery`, `ScanSort`, `ScanSummary` | [Queries and summaries](/docs/queries-summaries/) |
| Streaming | `StreamSession`, `StreamLimits`, `StreamError` | [Scanning modes](/docs/scanning-modes/#streaming) |
| Incremental | `IncrementalSession`, `IncrementalLimits`, `IncrementalError` | [Scanning modes](/docs/scanning-modes/#incremental-scanning) |
| Transformations | `transform` module | [Transformations](/docs/transformations/) |

## Build profiles

Rust Core uses `default-features = false`. The default configuration enables Flow transformations. Fuse adds `serde` and `parallel` to the default configuration.

These are feature combinations, not Cargo features named `core`, `flow`, or `fuse`.

The `transform` module is available only when its feature is enabled. Parallel scanning additionally requires the `parallel` feature.

## API documentation

The complete signatures and type-level documentation are available in [Rustdoc for Cribra 0.7.2](https://docs.rs/cribra/0.7.2/cribra/).

The guides on this site explain composition, semantics, and integration boundaries rather than duplicating every method signature.

For adapters, consult the [bindings matrix](/bindings/). Rust API availability does not imply identical adapter APIs or package publication.
