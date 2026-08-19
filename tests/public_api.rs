//! Integration tests for the public batch API.

use cribra::{Rule, ScanSort, Scanner, Severity};

fn report_for<'a>(
    scanner: &Scanner,
    key: &'a str,
    source: &'a str,
) -> cribra::ScanResults<&'a str> {
    scanner.scan([(key, source)])
}

#[test]
fn empty_builder_reports_no_findings() {
    let scanner = Scanner::builder().build().unwrap();
    let results = report_for(&scanner, "memory", "anything");

    assert_eq!(results.len(), 1);
    assert!(results.single_report().unwrap().is_empty());
}

#[test]
fn literal_rule_matches_multiple_occurrences() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("secret", "SECRET", Severity::High))
        .build()
        .unwrap();

    let results = report_for(&scanner, "memory", "SECRET xx SECRET");

    assert_eq!(results.single_report().unwrap().len(), 2);
}

#[test]
fn scanner_is_reusable() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("secret", "SECRET", Severity::High))
        .build()
        .unwrap();

    assert_eq!(
        report_for(&scanner, "one", "SECRET")
            .single_report()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        report_for(&scanner, "two", "nothing")
            .single_report()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        report_for(&scanner, "three", "SECRET SECRET")
            .single_report()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn batch_scan_preserves_keys_order_and_source_lengths() {
    let scanner = Scanner::builder().build().unwrap();
    let results = scanner.scan([("first", "αβ"), ("second", "plain"), ("third", "")]);

    assert_eq!(results.len(), 3);
    assert_eq!(results.as_slice()[0].key(), &"first");
    assert_eq!(results.as_slice()[0].source_bytes(), "αβ".len());
    assert_eq!(results.as_slice()[1].key(), &"second");
    assert_eq!(results.as_slice()[2].key(), &"third");
    assert!(results.single_report().is_none());
}

#[test]
fn unicode_columns_count_scalars() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("secret", "SECRET", Severity::High))
        .build()
        .unwrap();

    let results = report_for(&scanner, "memory", "😀😀 SECRET");
    let loc = results.single_report().unwrap().findings()[0].location();

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

    let results = report_for(&scanner, "memory", "github_pat_123");

    assert_eq!(results.single_report().unwrap().len(), 2);
}

#[test]
fn identical_needles_produce_multiple_findings() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("r1", "SECRET", Severity::Low))
        .rule(Rule::literal("r2", "SECRET", Severity::High))
        .build()
        .unwrap();

    let results = report_for(&scanner, "memory", "SECRET");

    assert_eq!(results.single_report().unwrap().len(), 2);
}

#[test]
fn results_flatten_findings_with_their_source_keys() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("secret", "SECRET", Severity::High))
        .build()
        .unwrap();

    let results = scanner.scan([
        ("a.env", "SECRET"),
        ("b.env", "clean"),
        ("c.env", "SECRET SECRET"),
    ]);

    let keys = results.findings().map(|(key, _)| *key).collect::<Vec<_>>();

    assert_eq!(keys, ["a.env", "c.env", "c.env"]);
}

#[test]
fn report_helpers_work() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("critical", "SECRET", Severity::Critical))
        .build()
        .unwrap();

    let results = report_for(&scanner, "memory", "SECRET");
    let report = results.single_report().unwrap();

    assert!(report.has_critical());
    assert_eq!(report.by_severity(Severity::Critical).count(), 1);
}

#[cfg(feature = "parallel")]
#[test]
fn parallel_scan_matches_serial_results_and_order() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("secret", "SECRET", Severity::High))
        .build()
        .unwrap();

    let inputs = [
        ("a.env", "SECRET"),
        ("b.env", "clean"),
        ("c.env", "SECRET SECRET"),
        ("d.env", "SECRET"),
    ];

    let serial = scanner.scan(inputs);
    let parallel = scanner.parallel_scan(inputs);

    assert_eq!(serial, parallel);
    assert_eq!(
        parallel
            .iter()
            .map(|entry| *entry.key())
            .collect::<Vec<_>>(),
        ["a.env", "b.env", "c.env", "d.env"],
    );
}

#[cfg(feature = "parallel")]
#[test]
fn parallel_scan_supports_owned_keys() {
    let scanner = Scanner::builder().build().unwrap();

    let inputs = vec![
        (String::from("one"), "alpha"),
        (String::from("two"), "beta"),
    ];

    let results = scanner.parallel_scan(inputs);

    assert_eq!(results.len(), 2);
    assert_eq!(results.as_slice()[0].key(), "one");
    assert_eq!(results.as_slice()[1].key(), "two");
}

#[test]
fn query_api_filters_sorts_and_exposes_convenience_helpers() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("zeta", "ZETA", Severity::High))
        .rule(Rule::literal("alpha", "ALPHA", Severity::Critical))
        .build()
        .unwrap();

    let results = scanner.scan([("b.env", "ZETA"), ("a.env", "ALPHA")]);

    let query = results.query().minimum_severity(Severity::High);

    assert!(query.has_matches());
    assert_eq!(query.count(), 2);

    let sorted = query.sort(ScanSort::RuleId);

    assert_eq!(sorted.first().unwrap().1.rule_id().as_str(), "alpha");
    assert_eq!(sorted.last().unwrap().1.rule_id().as_str(), "zeta");
}
