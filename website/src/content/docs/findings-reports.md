---
title: Findings and reports
description: Understand classifications, review candidates, explanations, and source coordinates.
order: 8
---

## Findings and review candidates

Cribra distinguishes two independent result categories.

**Findings** are rule-backed classifications. A `Finding` exposes its rule identity, location, severity, confidence, and optional remediation metadata.

**Sensitive candidates** are review evidence represented by `SensitiveCandidate`. They are not automatically promoted to findings and do not inherit finding semantics.

A source with no findings can still require review.

## Read a report

```rust
use cribra::{Rule, Scanner, Severity};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scanner = Scanner::builder()
        .rule(Rule::literal(
            "internal.marker",
            "PRIVATE_VALUE",
            Severity::High,
        ))
        .build()?;

    let results = scanner.scan([
        ("settings.txt", "marker=PRIVATE_VALUE"),
    ]);

    let report = results.single_report().expect("one source");

    for finding in report.findings() {
        println!(
            "{} at {}:{}",
            finding.rule_id(),
            finding.location().line(),
            finding.location().column(),
        );
    }

    println!("review candidates: {}", report.candidate_len());
    Ok(())
}
```

`ScanReport` exposes `findings()`, `candidates()`, `needs_review()`, `has_critical()`, and other report-level operations.

## Source coordinates

Finding and candidate spans use half-open UTF-8 byte ranges. Lines and columns are one-based, and columns count Unicode scalar values.

Byte offsets are not JavaScript string indices. Consumers must preserve coordinate semantics when projecting results into another language or UI.

## Explanations

`Finding::explanation(&scanner)` resolves classification information against the scanner that produced the finding.

`SensitiveCandidate::explanation()` exposes review evidence. Explanations project existing facts; they do not trigger a new classification.

## Source ownership

Public findings contain coordinates and metadata rather than copies of matched sensitive values. Applications retain responsibility for the original source.

Review-only candidates are not automatically transformed. See [explicit transformations](/docs/transformations/).

For ordered multi-source results, continue with [queries and summaries](/docs/queries-summaries/).
