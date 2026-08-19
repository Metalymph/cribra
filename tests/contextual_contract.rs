//! Regression contract for contextual built-in detection.
//!
//! These tests deliberately sit above individual matcher and validator units.
//! They freeze the public semantics established for Cribra 0.2:
//!
//! - common configuration syntaxes are semantically equivalent;
//! - contextual rules remain explicitly identifiable through public metadata;
//! - only the captured sensitive value becomes a finding span;
//! - placeholders and unrelated fields remain rejected;
//! - isolated opaque values are not promoted to contextual findings without
//!   evidence from a recognized surrounding key.
//!
//! Every credential below is synthetic and intentionally unusable.

use std::collections::BTreeMap;

use cribra::{Confidence, DetectionMode, Remediation, Scanner, Severity, builtins};

const AWS_SECRET: &str = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY";
const AZURE_SECRET: &str = "AbCdEfGhIjKlMnOpQrStUvWxYz0123456789";
const DATABASE_PASSWORD: &str = "CorrectHorseBatteryStaple!";
const SENSITIVE_HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const GENERIC_API_KEY: &str = "AbCdEfGhIjKlMnOpQrStUvWx";

fn contextual_scanner() -> Scanner {
    Scanner::builder()
        .builtins([
            builtins::AWS_SECRET_ACCESS_KEY,
            builtins::AZURE_CLIENT_SECRET,
            builtins::DATABASE_PASSWORD_FIELD,
            builtins::SENSITIVE_HASH,
            builtins::GENERIC_API_KEY,
        ])
        .build()
        .expect("contextual built-in rules must compile")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FindingContract {
    matched_value: String,
    severity: Severity,
    confidence: Confidence,
    remediation: Option<Remediation>,
}

fn semantic_contract(source: &str) -> BTreeMap<String, FindingContract> {
    let scanner = contextual_scanner();
    let results = scanner.scan([("fixture", source)]);
    let report = results.single_report().expect("one fixture was scanned");

    report
        .iter()
        .map(|finding| {
            let location = finding.location();
            let matched_value = source[location.start()..location.end()].to_owned();

            (
                finding.rule_id().as_str().to_owned(),
                FindingContract {
                    matched_value,
                    severity: finding.severity(),
                    confidence: finding.confidence(),
                    remediation: finding.remediation(),
                },
            )
        })
        .collect()
}

#[test]
fn contextual_rule_metadata_declares_contextual_detection() {
    let scanner = contextual_scanner();
    let metadata = scanner
        .rule_metadata()
        .map(|metadata| (metadata.id().to_owned(), metadata.detection_mode()))
        .collect::<BTreeMap<_, _>>();

    assert_eq!(metadata.len(), 5);
    assert_eq!(
        metadata.get("aws.secret-access-key"),
        Some(&DetectionMode::Contextual)
    );
    assert_eq!(
        metadata.get("azure.client-secret"),
        Some(&DetectionMode::Contextual)
    );
    assert_eq!(
        metadata.get("generic.database-password-field"),
        Some(&DetectionMode::Contextual)
    );
    assert_eq!(
        metadata.get("generic.sensitive-hash"),
        Some(&DetectionMode::Contextual)
    );
    assert_eq!(
        metadata.get("generic.api-key"),
        Some(&DetectionMode::Contextual)
    );
}

#[test]
fn contextual_semantics_are_format_invariant() {
    let baseline = semantic_contract(include_str!("fixtures/contextual.env"));

    assert_eq!(baseline.len(), 5);
    assert_eq!(baseline["aws.secret-access-key"].matched_value, AWS_SECRET);
    assert_eq!(baseline["azure.client-secret"].matched_value, AZURE_SECRET);
    assert_eq!(
        baseline["generic.database-password-field"].matched_value,
        DATABASE_PASSWORD
    );
    assert_eq!(
        baseline["generic.sensitive-hash"].matched_value,
        SENSITIVE_HASH
    );
    assert_eq!(baseline["generic.api-key"].matched_value, GENERIC_API_KEY);

    for (name, source) in [
        ("yaml", include_str!("fixtures/contextual.yaml")),
        ("toml", include_str!("fixtures/contextual.toml")),
        ("json", include_str!("fixtures/contextual.json")),
    ] {
        assert_eq!(
            semantic_contract(source),
            baseline,
            "{name} changed the public contextual finding semantics"
        );
    }
}

#[test]
fn contextual_false_positive_corpus_remains_empty() {
    let source = include_str!("fixtures/contextual-false-positives.txt");
    let scanner = Scanner::builder()
        .builtins([
            builtins::AWS_SECRET_ACCESS_KEY,
            builtins::AZURE_CLIENT_SECRET,
            builtins::PASSWORD_FIELD,
            builtins::DATABASE_PASSWORD_FIELD,
            builtins::SENSITIVE_HASH,
            builtins::GENERIC_API_KEY,
            builtins::GENERIC_AUTH_TOKEN,
            builtins::GENERIC_SECRET,
        ])
        .build()
        .expect("contextual built-in rules must compile");

    let results = scanner.scan([("fixture", source)]);
    let report = results.single_report().expect("one fixture was scanned");

    assert!(
        report.is_empty(),
        "contextual false-positive corpus produced findings: {:?}",
        report
            .iter()
            .map(|finding| finding.rule_id().as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn isolated_contextual_values_are_not_inferred_as_findings() {
    let source = [
        AWS_SECRET,
        AZURE_SECRET,
        DATABASE_PASSWORD,
        SENSITIVE_HASH,
        GENERIC_API_KEY,
    ]
    .join("\n");

    let scanner = contextual_scanner();
    let results = scanner.scan([("isolated-values", source.as_str())]);
    let report = results.single_report().expect("one fixture was scanned");

    assert!(
        report.is_empty(),
        "contextual values without recognized keys must not become findings: {:?}",
        report
            .iter()
            .map(|finding| finding.rule_id().as_str())
            .collect::<Vec<_>>()
    );
}
