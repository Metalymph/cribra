//! Query and sort findings without cloning the underlying reports.
//!
//! Run with:
//!
//! ```text
//! cargo run --example query_results
//! ```

use cribra::{Rule, ScanSort, Scanner, Severity};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scanner = Scanner::builder()
        .rule(Rule::prefix(
            "acme.critical-token",
            "crit_",
            Severity::Critical,
        ))
        .rule(Rule::prefix("acme.high-token", "high_", Severity::High))
        .rule(Rule::literal(
            "acme.medium-marker",
            "MEDIUM_SECRET",
            Severity::Medium,
        ))
        .build()?;

    let results = scanner.scan([
        ("service-b.env", "TOKEN=high_123456\n"),
        ("service-a.env", "TOKEN=crit_abcdef\n"),
        ("notes.txt", "MEDIUM_SECRET\n"),
    ]);

    let high_priority = results
        .query()
        .minimum_severity(Severity::High)
        .sort(ScanSort::Location);

    println!("high-priority findings: {}", high_priority.count());

    for (source, finding) in high_priority.iter() {
        println!(
            "{source}:{}:{} {} ({})",
            finding.location().line(),
            finding.location().column(),
            finding.rule_id(),
            finding.severity(),
        );
    }

    if let Some((source, finding)) = results.query().critical().first() {
        println!("first critical finding: {source} / {}", finding.rule_id());
    }

    Ok(())
}
