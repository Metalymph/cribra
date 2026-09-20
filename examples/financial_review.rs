use cribra::{Scanner, transform::redact};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Financial detection is deliberately opt-in. Compose it with Cribra's
    // default credential/security catalog when the application needs both.
    let scanner = Scanner::builder()
        .builtins(cribra::builtins::CURRENT)
        .builtins(cribra::builtins::financial::CURRENT)
        .build()?;

    // Synthetic data only. Applications retain ownership of their source text.
    let source = concat!(
        "service=checkout\n",
        "card_number=1234567890123452\n",
        "iban=IT60X0542811101000000123456\n",
        "log_level=info\n",
    );

    let results = scanner.scan([("checkout.env", source)]);
    let report = results.single_report().expect("one source was scanned");

    println!(
        "classified {} finding(s); {} candidate(s) require review",
        report.len(),
        report.candidate_len()
    );

    // Findings expose classification metadata and coordinates, not matched
    // sensitive values.
    for finding in report.findings() {
        println!(
            "{} at {}:{} — severity={:?}, confidence={:?}",
            finding.rule_id(),
            finding.location().line(),
            finding.location().column(),
            finding.severity(),
            finding.confidence(),
        );
    }

    // Transformations operate from authoritative finding spans. Review-only
    // candidates are never transformed automatically.
    let safe_copy = redact(source, report)?;

    println!("\nSafe derivative:\n{safe_copy}");

    Ok(())
}
