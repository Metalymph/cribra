//! Integration tests for the public API.

use silens_scan::{Rule, Scanner, Severity};

#[test]
fn empty_scanner_has_no_findings() {
    let scanner = Scanner::default();
    assert!(scanner.scan("anything").is_empty());
}

#[test]
fn literal_rule_matches_multiple_occurrences() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("secret", "SECRET", Severity::High))
        .build()
        .unwrap();

    assert_eq!(scanner.scan("SECRET xx SECRET").len(), 2);
}

#[test]
fn scanner_is_reusable() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("secret", "SECRET", Severity::High))
        .build()
        .unwrap();

    assert_eq!(scanner.scan("SECRET").len(), 1);
    assert_eq!(scanner.scan("nothing").len(), 0);
    assert_eq!(scanner.scan("SECRET SECRET").len(), 2);
}

#[test]
fn unicode_columns_count_scalars() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("secret", "SECRET", Severity::High))
        .build()
        .unwrap();

    let report = scanner.scan("😀😀 SECRET");
    let loc = report.findings()[0].location();

    assert_eq!(loc.line(), 1);
    assert_eq!(loc.column(), 4);
}

#[test]
fn overlapping_literals_are_reported() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("short", "github", Severity::Low))
        .rule(Rule::literal("long", "github_pat", Severity::High))
        .build()
        .unwrap();

    assert_eq!(scanner.scan("github_pat_123").len(), 2);
}

#[test]
fn identical_needles_produce_multiple_findings() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("r1", "SECRET", Severity::Low))
        .rule(Rule::literal("r2", "SECRET", Severity::High))
        .build()
        .unwrap();

    assert_eq!(scanner.scan("SECRET").len(), 2);
}

#[test]
fn report_helpers_work() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("critical", "SECRET", Severity::Critical))
        .build()
        .unwrap();

    let report = scanner.scan("SECRET");
    assert!(report.has_critical());
    assert_eq!(report.by_severity(Severity::Critical).count(), 1);
}
