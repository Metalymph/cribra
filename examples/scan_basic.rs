//! Minimal end-to-end scan using custom rules.
//!
//! Run with:
//!
//! ```text
//! cargo run --example scan_basic
//! ```

use cribra::{Remediation, Rule, Scanner, Severity};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scanner = Scanner::builder()
        .rule(
            Rule::prefix("acme.api-key", "acme_live_", Severity::Critical)
                .with_remediation(Remediation::RotateCredential),
        )
        .rule(
            Rule::literal("internal.marker", "PRIVATE_VALUE", Severity::High)
                .with_remediation(Remediation::RemoveSensitiveValue),
        )
        .build()?;

    let sources = [
        (
            "config.env",
            "ACME_API_KEY=acme_live_7f3a91\nMODE=production\n",
        ),
        (
            "settings.toml",
            "marker = \"PRIVATE_VALUE\"\nlog_level = \"info\"\n",
        ),
        ("clean.txt", "Nothing sensitive here.\n"),
    ];

    let results = scanner.scan(sources);

    println!("{}", results.summary());

    for (source, finding) in results.findings() {
        println!(
            "{source}:{}:{} [{} / {}] {}",
            finding.location().line(),
            finding.location().column(),
            finding.severity(),
            finding.confidence(),
            finding.rule_id(),
        );

        if let Some(remediation) = finding.remediation() {
            println!("  remediation: {}", remediation.message());
        }
    }

    Ok(())
}
