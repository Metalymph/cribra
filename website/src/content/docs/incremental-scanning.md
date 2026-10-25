---
title: Incremental scanning
description: Revisioned UTF-8 documents, atomic edits, snapshots, budgets, and failure recovery.
order: 18
---

## Contract and use cases

Incremental scanning maintains a **Core-owned, complete UTF-8 document** and its current report. It is designed for edited content, such as an editor buffer, rather than append-only transport. `Scanner::incremental_session(String)` creates a revisioned session; `incremental_session_with_limits` accepts `IncrementalLimits`. Built-in portfolios are supported independently of streaming admission rules.

## Rust session setup

The repository's [incremental contract](https://github.com/Metalymph/cribra/blob/main/docs/incremental.md) documents a session initialized with explicit limits:

```rust
use cribra::{IncrementalLimits, Rule, Scanner, Severity};

let limits = IncrementalLimits::default()
    .with_max_candidates(Some(10_000))
    .with_max_retained_cache_records(Some(20_000))
    .with_max_materialized_results(Some(10_000))
    .with_max_transaction_storage_bytes(Some(8 * 1024 * 1024));
```

Use the exact edit and snapshot method signatures in the [Core contract](https://github.com/Metalymph/cribra/blob/main/docs/incremental.md) for your checkout. Do not translate the Rust byte-range contract into character indices in a UI adapter.

## Edits, revisions and snapshots

An edit replaces a **half-open UTF-8 byte range** using the caller's expected revision. Insertions, deletions and replacements use the same operation; endpoints must be valid UTF-8 scalar boundaries. Successful edits commit a new revision and a complete report with the same semantics as a fresh whole-source scan of the resulting document.

A stale revision, invalid range or invalid UTF-8 boundary is a recoverable edit rejection: correct the request and retry against the current committed revision. Resource-budget and internal invariant failures are terminal for further edits, while preserving the last successfully committed complete snapshot. Do not interpret a rejected edit as a partially committed change.

## Exactness versus speed

Incremental is not approximate detection. Literal and structural matchers can localize rescanning; prefix, suffix and regex rules can require exact whole-rule replay. This is distinct from streaming compatibility and does not promise that every edit is cheaper than a fresh scan. Performance depends on the ruleset, edit pattern and source size.

## Optional budgets and memory

The four independently optional limits cover raw candidate emissions, retained cache records, materialized results and logical transaction storage. Incremental result accounting differs from streaming's retained-result accounting; do not transfer limits between modes by name alone. The session owns the **entire document**, so these budgets are **not** a total heap cap or a promise of bounded document retention.

## Integration boundaries

Rust, C ABI, UniFFI and source WASM expose documented incremental interfaces; their ownership and snapshot representations differ. In particular, source WASM revisions use exact `bigint` values, and native adapters have their own lifetime contracts. The CLI does not offer incremental-document commands. Source capability does not prove equivalent functionality in a published WASM package.

See [scanning modes](/docs/scanning-modes/), the [incremental Core contract](https://github.com/Metalymph/cribra/blob/main/docs/incremental.md), [C ABI contract](https://github.com/Metalymph/cribra/blob/main/docs/INTEROP.md), and [compatibility and publication status](https://github.com/Metalymph/cribra/blob/main/docs/COMPATIBILITY.md).
