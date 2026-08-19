#![cfg(feature = "serde")]

use cribra::{
    Confidence, Remediation, Rule, ScanResults, Scanner, Severity,
    transform::{ShareBundle, ShareMode, ShareModeKind},
};

#[test]
fn finding_json_is_structured_and_never_contains_the_matched_secret() {
    let scanner = Scanner::builder()
        .rule(
            Rule::literal("example.secret", "SECRET", Severity::Critical)
                .with_remediation(Remediation::RevokeAndRotateCredential),
        )
        .build()
        .unwrap();

    let results = scanner.scan([(String::from("memory"), "TOKEN=SECRET")]);
    let json = serde_json::to_string_pretty(&results).unwrap();

    assert!(json.contains(r#""severity": "critical""#));
    assert!(json.contains(r#""confidence": "high""#));
    assert!(json.contains(r#""remediation": "revoke_and_rotate_credential""#));
    assert!(json.contains(r#""rule_id": "example.secret""#));

    // ScanResults records source metadata and findings, never source text or
    // the matched value itself.
    assert!(!json.contains("TOKEN=SECRET"));
    assert!(!json.contains(r#""SECRET""#));
}

#[test]
fn scan_results_round_trip_without_losing_public_data() {
    let scanner = Scanner::builder()
        .rule(
            Rule::literal("example.secret", "SECRET", Severity::High)
                .with_remediation(Remediation::RemoveSensitiveValue),
        )
        .build()
        .unwrap();

    let original = scanner.scan([
        (String::from("a.env"), "A=SECRET"),
        (String::from("clean.env"), "nothing here"),
    ]);

    let json = serde_json::to_string(&original).unwrap();
    let decoded: ScanResults<String> = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded, original);
}

#[test]
fn enum_wire_names_are_stable_snake_case() {
    assert_eq!(
        serde_json::to_string(&Severity::Critical).unwrap(),
        r#""critical""#,
    );
    assert_eq!(
        serde_json::to_string(&Confidence::High).unwrap(),
        r#""high""#,
    );
    assert_eq!(
        serde_json::to_string(&Remediation::RotatePassword).unwrap(),
        r#""rotate_password""#,
    );
    assert_eq!(
        serde_json::to_string(&ShareModeKind::Pseudonymize).unwrap(),
        r#""pseudonymize""#,
    );
}

#[test]
fn share_bundle_round_trip_preserves_safe_transformed_content_and_manifest() {
    let scanner = Scanner::builder()
        .rule(Rule::literal(
            "example.secret",
            "SECRET",
            Severity::Critical,
        ))
        .build()
        .unwrap();

    let source = "TOKEN=SECRET";
    let results = scanner.scan([(String::from("memory"), source)]);

    let bundle = ShareBundle::builder()
        .mode(ShareMode::Redact)
        .build(&results, [source])
        .unwrap();

    let json = serde_json::to_string_pretty(&bundle).unwrap();

    assert!(json.contains("[REDACTED]"));
    assert!(json.contains(r#""mode": "redact""#));
    assert!(!json.contains(r#""SECRET""#));

    let decoded: ShareBundle<String> = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, bundle);
}
