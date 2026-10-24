---
title: Whole-source scanning
description: Complete deterministic scans, ordered source reports, and reference semantics.
order: 16
---

## When to use it

Whole-source scanning is the semantic reference for Cribra's supported scanning modes. Supply complete UTF-8 sources to an immutable, reusable `Scanner`; each scan returns complete results. This is the simplest mode when the entire source is already available, and it supports built-in portfolios without streaming admission restrictions.

## Rust example

```rust
use cribra::Scanner;

let results = Scanner::default().scan([
    ("settings.toml", "log_level = \"info\""),
]);
for (source, finding) in results.findings() {
    println!("{source}: {}", finding.rule_id());
}
```

`ScanResults` preserves source order, provides per-source reports, queries and aggregate summaries. Confirmed `Finding` records and review-only `SensitiveCandidate` records are distinct; a report without findings can still require review.

## Coordinates and ownership

Finding and candidate spans use half-open UTF-8 **byte** offsets. Lines and columns are one-based, with columns measured in Unicode scalar values. A byte offset is not a JavaScript string index. The caller owns source acquisition and retention; public findings expose classification metadata and coordinates rather than matched source values.

## Determinism and parallelism

Equivalent supported operations preserve deterministic classification and report ordering. The optional Rust `parallel` feature distributes **independent sources**, not pieces of a single logical source, and preserves input order. Rust Core/Flow/Fuse are feature combinations, not Cargo features literally named `core`, `flow` or `fuse`.

## Related guides

Read [scanner configuration](/docs/scanner-configuration/), [findings and reports](/docs/findings-reports/), [queries and summaries](/docs/queries-summaries/), and [streaming](/docs/streaming-scanning/) or [incremental scanning](/docs/incremental-scanning/) when your source lifecycle differs.

The repository's [basic example](https://github.com/Metalymph/cribra/blob/main/examples/scan_basic.rs) and [architecture contract](https://github.com/Metalymph/cribra/blob/main/docs/ARCHITECTURE.md) are the authoritative implementation references.
