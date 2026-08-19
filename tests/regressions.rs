//! Regression tests for public scanner invariants.

use cribra::{Rule, Scanner, Severity};

#[test]
fn distinct_rule_ids_with_identical_needles_remain_distinct() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("first", "SECRET", Severity::Low))
        .rule(Rule::literal("second", "SECRET", Severity::Critical))
        .build()
        .expect("scanner should build");

    let results = scanner.scan([("fixture", "SECRET")]);
    let report = results.single_report().expect("one report");

    assert_eq!(report.len(), 2);

    let ids = report
        .iter()
        .map(|finding| finding.rule_id().as_str())
        .collect::<Vec<_>>();

    assert!(ids.contains(&"first"));
    assert!(ids.contains(&"second"));
}

#[test]
fn partial_overlaps_are_not_collapsed() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("short", "github", Severity::Low))
        .rule(Rule::literal("long", "github_pat", Severity::High))
        .build()
        .expect("scanner should build");

    let results = scanner.scan([("fixture", "github_pat_value")]);
    let report = results.single_report().expect("one report");

    assert_eq!(report.len(), 2);
}

#[test]
fn batch_results_preserve_input_order_with_empty_reports_between_matches() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("secret", "SECRET", Severity::High))
        .build()
        .expect("scanner should build");

    let results = scanner.scan([
        ("first", "SECRET"),
        ("clean", "nothing"),
        ("third", "SECRET"),
    ]);

    let keys = results.iter().map(|entry| *entry.key()).collect::<Vec<_>>();

    assert_eq!(keys, ["first", "clean", "third"]);
    assert_eq!(results.clean().count(), 1);
    assert_eq!(results.failed().count(), 2);
}

#[test]
fn unicode_before_match_does_not_corrupt_byte_or_column_contract() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("secret", "SECRET", Severity::High))
        .build()
        .expect("scanner should build");

    let source = "😀é SECRET";
    let results = scanner.scan([("unicode", source)]);
    let finding = &results.single_report().expect("one report").findings()[0];
    let location = finding.location();

    assert_eq!(&source[location.start()..location.end()], "SECRET");
    assert_eq!(location.line(), 1);
    assert_eq!(location.column(), 4);
}
