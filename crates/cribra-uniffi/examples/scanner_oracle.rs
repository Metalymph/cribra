//! Test-only JSON oracle from Core, independent of the adapter conversions.
//! No source text or matched values are written to the snapshot.

#[path = "../tests/support/configuration.rs"]
mod configuration;
#[path = "support/incremental.rs"]
mod incremental;
#[path = "support/stream.rs"]
mod stream;
#[path = "../tests/support/mod.rs"]
mod support;

use cribra_uniffi::{ScanSource, SourceId};
use serde_json::{Value, json};

#[path = "../tests/support/metadata.rs"]
mod metadata;

fn scan_snapshot(scanner: &cribra::Scanner, sources: &[ScanSource]) -> Value {
    let results = scanner.scan(
        sources
            .iter()
            .map(|source| (source.id.value.as_str(), source.text.as_str())),
    );
    let reports: Vec<Value> = results
        .iter()
        .map(|entry| metadata::report(scanner, entry.key(), entry.source_bytes(), entry.report()))
        .collect();
    let summary = results.summary();
    json!({
        "reports": reports,
        "summary": {
            "scannedSources": summary.scanned_sources(),
            "scannedBytes": summary.scanned_bytes(),
            "reportsWithFindings": summary.reports_with_findings(),
            "reportsWithoutFindings": summary.reports_without_findings(),
            "totalFindings": summary.total_findings(),
            "totalCandidates": summary.total_candidates(),
            "reportsWithCandidates": summary.reports_with_candidates(),
            "critical": summary.critical(), "high": summary.high(),
            "medium": summary.medium(), "low": summary.low(), "info": summary.info(),
        },
    })
}

fn main() {
    let mut snapshot = scan_snapshot(&cribra::Scanner::default(), &support::sources());
    let configurations: Vec<Value> = configuration::cases().into_iter().map(|case| {
        match configuration::core_build(&case.config) {
            Ok(scanner) => {
                let source = ScanSource { id: SourceId { value: case.name.clone() }, text: case.source };
                let rules: Vec<Value> = scanner.rule_metadata().map(|rule| json!({
                    "id": rule.id(), "kind": format!("{:?}", rule.kind()),
                    "detectionMode": format!("{:?}", rule.detection_mode()),
                    "severity": rule.severity().to_string(),
                    "remediation": rule.remediation().map(|r| json!({"label": r.label(), "message": r.message()})),
                })).collect();
                json!({"name": case.name, "rules": rules, "results": scan_snapshot(&scanner, &[source])})
            }
            Err(error) => json!({"name": case.name, "error": error}),
        }
    }).collect();
    snapshot["configurations"] = json!(configurations);
    snapshot["streamErrors"] = stream::errors();
    snapshot["incremental"] = incremental::data();
    println!("{snapshot}");
}
