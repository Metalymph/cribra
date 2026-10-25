---
title: Scanner and configuration
description: Build immutable scanners, compose catalogs, and handle configuration errors.
order: 6
---

## Default scanner

`Scanner::default()` constructs a scanner using Cribra's current built-in catalog.

```rust
use cribra::Scanner;

let scanner = Scanner::default();
let results = scanner.scan([("example.txt", "synthetic input")]);
println!("{}", results.summary());
```

The default catalog is not a promise to detect every secret or sensitive value.

## Custom scanner

Use `Scanner::builder()` to construct an explicit ruleset.

```rust
use cribra::{Rule, Scanner, Severity};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scanner = Scanner::builder()
        .rule(Rule::prefix(
            "acme.api-key",
            "acme_live_",
            Severity::Critical,
        ))
        .build()?;

    let results = scanner.scan([
        ("config.env", "TOKEN=acme_live_123456"),
    ]);

    println!("{}", results.summary());
    Ok(())
}
```

This follows the repository's runnable [basic scanning example](https://github.com/Metalymph/cribra/blob/main/examples/scan_basic.rs).

## Compose built-in catalogs

The current catalog is available as `builtins::CURRENT`. Financial and personal catalogs are explicit opt-ins.

```rust
use cribra::{Scanner, builtins};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scanner = Scanner::builder()
        .builtins(builtins::CURRENT)
        .builtins(builtins::financial::CURRENT)
        .build()?;

    println!("{} compiled rules", scanner.rules_count());
    Ok(())
}
```

Adding an opt-in catalog does not silently enable other catalogs. The [financial review example](https://github.com/Metalymph/cribra/blob/main/examples/financial_review.rs) demonstrates this composition.

## Configuration errors

`ScannerBuilder::build()` returns `Result<Scanner, ScannerBuildError>`. Handle invalid configurations explicitly rather than assuming every ruleset compiles.

`ScannerBuilder` exposes `builtin`, `builtins`, `rule`, and `rules` for composing specifications and custom rules.

Once built, a scanner's compiled configuration is immutable and reusable. Source-local execution state belongs to individual scans or sessions.

## Inspect active metadata

`Scanner::rules_count()`, `Scanner::is_empty()`, and `Scanner::rule_metadata()` provide inspection of the compiled configuration.

Metadata describes the compiled rules; it is not a replacement for scan results or application policy.

See [rules and built-ins](/docs/rules-builtins/) and the [Rust API reference](/docs/api-reference/).
