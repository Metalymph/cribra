//! Direct-Core Share Bundle oracle for generated Flow runtime checks.
#[allow(dead_code)]
mod oracle_support;
use std::time::UNIX_EPOCH;

use cribra::ScanSummary;
use cribra::transform::{
    PseudonymizationOptions, ShareBundle, ShareMode, SynthesisOptions, TransformError,
};
use serde_json::{Value, json};

fn summary(summary: ScanSummary) -> Value {
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

fn mode(case: &Value) -> Option<ShareMode> {
    match case["mode"].as_str() {
        Some("redact") => Some(ShareMode::Redact),
        Some("template") => Some(ShareMode::Template),
        Some("pseudonymize") => {
            let key = [case["keyByte"].as_u64().unwrap_or(7) as u8; 32];
            let mut options = PseudonymizationOptions::new(key);
            if let Some(prefix) = case["prefix"].as_str() {
                options = options.prefix(prefix);
            }
            if let Some(digest_bytes) = case["digestBytes"].as_u64() {
                options = options.digest_bytes(digest_bytes as usize);
            }
            Some(ShareMode::Pseudonymize(options))
        }
        Some("synthesize") => {
            let key = [case["keyByte"].as_u64().unwrap_or(7) as u8; 32];
            let mut options = SynthesisOptions::new(key);
            if let Some(marker) = case["marker"].as_str() {
                options = options.marker(marker);
            }
            Some(ShareMode::Synthesize(options))
        }
        Some(other) => panic!("unknown share mode: {other}"),
        None => None,
    }
}

fn outcome(name: &Value, result: Result<ShareBundle<&str>, TransformError>) -> Value {
    match result {
        Ok(bundle) => {
            let manifest = bundle.manifest();
            let time = manifest
                .generated_at()
                .duration_since(UNIX_EPOCH)
                .expect("oracle share bundle timestamp should be after the Unix epoch");
            let sources = bundle
                .sources()
                .iter()
                .map(|source| {
                    json!({
                        "id":source.key(),
                        "content":source.content()
                    })
                })
                .collect::<Vec<_>>();

            json!({
                "name":name,
                "sources":sources,
                "manifest":{
                    "mode":format!("{:?}", manifest.mode()),
                    "summary":summary(manifest.summary()),
                    "generatedAtSeconds":time.as_secs(),
                    "generatedAtNanoseconds":time.subsec_nanos()
                }
            })
        }
        Err(error) => json!({
            "name":name,
            "error":error.to_string()
        }),
    }
}

fn main() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("../tests/runtime/share-cases.json")).unwrap();
    let mut output = Vec::new();

    for case in cases {
        let scanner = oracle_support::scanner(&case);
        let sources = case["sources"].as_array().unwrap();
        let inputs = sources
            .iter()
            .map(|source| {
                (
                    source["id"].as_str().unwrap(),
                    source["source"].as_str().unwrap(),
                )
            })
            .collect::<Vec<_>>();
        let builder = ShareBundle::builder();

        let result = match mode(&case) {
            Some(mode) => builder.mode(mode).scan_and_build(&scanner, inputs),
            None => builder.scan_and_build(&scanner, inputs),
        };

        output.push(outcome(&case["name"], result));
    }

    println!("{}", json!(output));
}
