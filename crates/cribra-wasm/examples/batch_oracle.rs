//! Direct-Core sequential batch oracle for generated Core and Flow runtime checks.
mod oracle_support;
use oracle_support::report;
use serde_json::{Value, json};

/// Direct-Core aggregate counters for one complete ordered batch.
fn summary(summary: cribra::ScanSummary) -> Value {
    json!({
        "scannedSources":summary.scanned_sources(),
        "scannedBytes":summary.scanned_bytes(),
        "reportsWithFindings":summary.reports_with_findings(),
        "reportsWithoutFindings":summary.reports_without_findings(),
        "totalFindings":summary.total_findings(),
        "totalCandidates":summary.total_candidates(),
        "reportsWithCandidates":summary.reports_with_candidates(),
        "hasCandidates":summary.has_candidates(),
        "critical":summary.critical(),
        "high":summary.high(),
        "medium":summary.medium(),
        "low":summary.low(),
        "info":summary.info(),
        "isClean":summary.is_clean(),
        "hasCritical":summary.has_critical()
    })
}

fn main() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("../tests/runtime/batch-cases.json")).unwrap();
    let mut output = Vec::new();

    for case in cases {
        let scanner = oracle_support::scanner(&case);
        let sources = case["sources"].as_array().unwrap();
        let results = scanner.scan(
            sources
                .iter()
                .map(|source| {
                    (
                        source["id"].as_str().unwrap(),
                        source["source"].as_str().unwrap(),
                    )
                })
                .collect::<Vec<_>>(),
        );
        let aggregate = summary(results.summary());
        let entries = results
            .iter()
            .map(|entry| {
                json!({
                    "id":entry.key(),
                    "report":report(&scanner, entry.report(), entry.source_bytes())
                })
            })
            .collect::<Vec<_>>();

        output.push(json!({
            "name":case["name"],
            "entries":entries,
            "summary":aggregate
        }));
    }

    println!("{}", json!(output));
}
