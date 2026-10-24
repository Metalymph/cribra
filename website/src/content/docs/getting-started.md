---
title: Getting started
description: Add Cribra to a Rust application, scan caller-owned text, and choose an integration.
order: 1
---

## Add the Rust crate

Cribra Core 0.7.2 is published on crates.io according to the repository's [compatibility audit](https://github.com/Metalymph/cribra/blob/main/docs/COMPATIBILITY.md). The crate requires Rust 1.97 or newer.

```toml title="Cargo.toml"
[dependencies]
cribra = "0.7.2"
```

The default configuration is **Flow**, with transformations enabled. **Core** uses `default-features = false`; **Fuse** adds `features = ["serde", "parallel"]` to the default configuration. These names describe Rust feature combinations, not Cargo features named `core`, `flow`, or `fuse`.

## Scan your first source

This excerpt comes from the runnable [scan_basic example](https://github.com/Metalymph/cribra/blob/main/examples/scan_basic.rs). The custom prefix and input are synthetic.

```rust
use cribra::{Rule, Scanner, Severity};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scanner = Scanner::builder()
        .rule(Rule::prefix("acme.api-key", "acme_live_", Severity::Critical))
        .build()?;

    let results = scanner.scan([("config.env", "ACME_API_KEY=acme_live_7f3a91\n")]);
    for (source, finding) in results.findings() {
        println!("{source}: {}", finding.rule_id());
    }
    Ok(())
}
```

For built-in detection, use `Scanner::default()`. Financial and personal catalogs are opt-in; see the [financial review example](https://github.com/Metalymph/cribra/blob/main/examples/financial_review.rs) for explicit catalog composition.

Your application supplies source identifiers and text. Cribra performs synchronous processing without filesystem discovery, network access, or an async runtime.

## Use the CLI from this repository

```sh
cargo build -p cribra-cli
target/debug/cribra scan examples/fixtures/inputs/credentials.env --format json
```

Run these commands from the repository root. On Windows, the executable is `target/debug/cribra.exe`. A successful scan exits with code 0 even when it finds sensitive data or review candidates; applications choose their own failure policy. CLI registry publication is not established by the audit.

## Choose another language

The [bindings matrix](/bindings/) separates source availability, capability coverage, qualification, and distribution. Swift, Python, and Kotlin/JVM consumers are implemented, but their local or CI artifacts do not establish remote package publication.

Continue with [core concepts](/docs/core-concepts/), [transformations](/docs/transformations/), and [scanning modes](/docs/scanning-modes/). Detailed build prerequisites live in the repository's [getting-started guide](https://github.com/Metalymph/cribra/blob/main/docs/GETTING_STARTED.md).
