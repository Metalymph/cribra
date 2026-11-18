---
title: Queries and summaries
description: Filter, sort, and aggregate deterministic scan results.
order: 9
---

## Multi-source results

`Scanner::scan()` returns `ScanResults<K>`, preserving the input source order.

Each `ScanEntry<K>` contains a caller-provided key, source byte count, and `ScanReport`.

`ScanResults` exposes `findings()`, `candidates()`, `summary()`, and source-level partitions including `failed()`, `review()`, and `clean()`.

## Filter and sort findings

```rust
use cribra::{Rule, ScanSort, Scanner, Severity};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scanner = Scanner::builder()
        .rule(Rule::prefix(
            "acme.critical-token",
            "crit_",
            Severity::Critical,
        ))
        .build()?;

    let results = scanner.scan([
        ("service.env", "TOKEN=crit_123456"),
    ]);

    let selected = results
        .query()
        .minimum_severity(Severity::High)
        .sort(ScanSort::Location);

    for (source, finding) in selected.iter() {
        println!("{source}: {}", finding.rule_id());
    }

    Ok(())
}
```

This is based on the runnable [query example](https://github.com/Metalymph/cribra/blob/main/examples/query_results.rs).

## Query operations

`ScanQuery` supports filters by severity, minimum severity, confidence, minimum confidence, and rule identity.

Convenience filters include `critical()`, `high_priority()`, and `high_confidence()`.

Queries expose `iter()`, `count()`, `first()`, `last()`, and `collect()`. `sort()` produces `SortedScanQuery`.

Query operations concern classified findings. Review candidates remain available through their separate report and results APIs.

## Aggregate summaries

`ScanResults::summary()` returns `ScanSummary`.

The summary includes source counts, scanned bytes, finding counts, candidate counts, and severity aggregates.

```rust
use cribra::Scanner;

let results = Scanner::default().scan([
    ("one.txt", "synthetic text"),
    ("two.txt", "another source"),
]);

let summary = results.summary();

println!("sources: {}", summary.scanned_sources());
println!("findings: {}", summary.total_findings());
println!("candidates: {}", summary.total_candidates());
```

Applications decide how to interpret these values and whether a result should block a workflow.

See [findings and reports](/docs/findings-reports/) and the [Rust API reference](/docs/api-reference/).
