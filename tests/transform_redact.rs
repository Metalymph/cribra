//! Public transformation tests.

use silens_scan::{
    Redaction, Rule, Scanner, Severity,
    transform::{redact, redact_with},
};

#[test]
fn public_redaction_uses_scanner_spans() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("secret", "SECRET", Severity::High))
        .build()
        .expect("scanner should build");

    let source = "before SECRET after";
    let results = scanner.scan([("memory", source)]);
    let report = results.single_report().expect("one report");

    assert_eq!(redact(source, report).unwrap(), "before [REDACTED] after",);
}

#[test]
fn public_redaction_supports_custom_markers() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("secret", "SECRET", Severity::High))
        .build()
        .expect("scanner should build");

    let source = "TOKEN=SECRET";
    let results = scanner.scan([("memory", source)]);
    let report = results.single_report().expect("one report");

    assert_eq!(
        redact_with(source, report, &Redaction::new("<hidden>")).unwrap(),
        "TOKEN=<hidden>",
    );
}

#[test]
fn overlapping_public_findings_cannot_leak_partial_secret_text() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("short", "github", Severity::Low))
        .rule(Rule::literal("long", "github_pat", Severity::High))
        .build()
        .expect("scanner should build");

    let source = "github_pat_value";
    let results = scanner.scan([("memory", source)]);
    let report = results.single_report().expect("one report");

    let transformed = redact(source, report).unwrap();

    assert_eq!(transformed, "[REDACTED]_value");
    assert!(!transformed.contains("github"));
}
