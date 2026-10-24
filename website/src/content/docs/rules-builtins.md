---
title: Rules and built-ins
description: Compose literal, prefix, suffix, and pattern rules with explicit catalogs.
order: 7
---

## Rule families

Cribra supports literal, prefix, suffix, and regex-based pattern rules.

`Rule` represents application-defined rules. `RuleSpec` provides built-in-style specifications with static identifiers and values.

The available constructors include:

- `Rule::literal(...)`
- `Rule::prefix(...)`
- `Rule::suffix(...)`
- `Rule::pattern(...)`

Pattern construction is fallible; handle its error before building the scanner.

## Custom rules

```rust
use cribra::{Remediation, Rule, Scanner, Severity};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scanner = Scanner::builder()
        .rule(
            Rule::prefix(
                "acme.service-token",
                "acme_live_",
                Severity::Critical,
            )
            .with_remediation(Remediation::RotateCredential),
        )
        .build()?;

    let results = scanner.scan([
        ("config.env", "TOKEN=acme_live_123456"),
    ]);

    println!("{}", results.summary());
    Ok(())
}
```

The full [custom rules example](https://github.com/Metalymph/cribra/blob/main/examples/custom_rules.rs) also demonstrates pattern rules and explanations.

## Built-in catalogs

`builtins::CURRENT` is the default built-in catalog.

Additional catalogs are selected explicitly:

- `builtins::financial::CURRENT`
- `builtins::personal::CURRENT`

Use `Scanner::builder().builtins(...)` to compose catalogs according to application requirements.

## Validation and classification

Built-in rules may apply deterministic or contextual validation. Custom rules remain matcher-authoritative and cannot select private built-in validators.

`RuleMetadata` and `DetectionMode` expose classification metadata without performing a second detection pass.

Rule identity, configuration validation, and collisions remain Core-owned behavior. Applications must handle construction errors instead of silently discarding problematic rules.

No catalog guarantees complete detection of all sensitive values.

Continue with [findings and reports](/docs/findings-reports/).
