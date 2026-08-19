#![cfg(feature = "serde")]

use cribra::{Rule, Scanner, Severity};

const CLASSIFIED_SECRET: &str = "KNOWN_PRIVATE_SECRET_VALUE";
const AMBIGUOUS_VALUE: &str = "ABCD-EFGH-IJKL-MNOP";

#[test]
fn serialized_scan_results_remain_metadata_only() {
    let scanner = Scanner::builder()
        .rule(Rule::literal(
            "organization.private-value",
            CLASSIFIED_SECRET,
            Severity::Critical,
        ))
        .build()
        .expect("scanner should compile");

    let results = scanner.scan([
        ("classified.env", CLASSIFIED_SECRET),
        ("review.txt", AMBIGUOUS_VALUE),
    ]);

    let json = serde_json::to_string_pretty(&results).expect("serialize scan results");

    assert!(json.contains("\"findings\""));
    assert!(json.contains("\"candidates\""));
    assert!(json.contains("\"source_bytes\""));

    assert!(!json.contains(CLASSIFIED_SECRET));
    assert!(!json.contains(AMBIGUOUS_VALUE));
    assert!(!json.contains("\"source\""));
    assert!(!json.contains("\"matched_value\""));
    assert!(!json.contains("\"explanation\""));
}

#[test]
fn serialized_results_round_trip_without_source_material() {
    let scanner = Scanner::builder()
        .rule(Rule::literal(
            "organization.private-value",
            CLASSIFIED_SECRET,
            Severity::High,
        ))
        .build()
        .expect("scanner should compile");

    let results = scanner.scan([
        ("classified.env", CLASSIFIED_SECRET),
        ("review.txt", AMBIGUOUS_VALUE),
    ]);

    let json = serde_json::to_string(&results).expect("serialize scan results");

    let decoded: cribra::ScanResults<String> =
        serde_json::from_str(&json).expect("deserialize scan results");

    assert_eq!(decoded.len(), 2);
    assert_eq!(decoded.total_findings(), 1);
    assert_eq!(decoded.total_candidates(), 1);

    let classified = decoded
        .iter()
        .find(|entry| entry.key() == "classified.env")
        .expect("classified entry");

    assert_eq!(classified.report().findings().len(), 1);
    assert!(classified.report().candidates().is_empty());

    let review = decoded
        .iter()
        .find(|entry| entry.key() == "review.txt")
        .expect("review entry");

    assert!(review.report().findings().is_empty());
    assert_eq!(review.report().candidates().len(), 1);

    assert!(!json.contains(CLASSIFIED_SECRET));
    assert!(!json.contains(AMBIGUOUS_VALUE));
}
