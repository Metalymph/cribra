//! Hardening tests for the public query API.

use silens_scan::{Rule, ScanSort, Scanner, Severity};

fn scanner() -> Scanner {
    Scanner::builder()
        .rule(Rule::literal("alpha", "ALPHA", Severity::Critical))
        .rule(Rule::literal("beta", "BETA", Severity::High))
        .rule(Rule::literal("gamma", "GAMMA", Severity::Medium))
        .rule(Rule::literal("delta", "DELTA", Severity::Low))
        .build()
        .expect("hardening scanner should build")
}

#[test]
fn filter_order_is_independent_across_distinct_dimensions() {
    let scanner = scanner();
    let results = scanner.scan([("fixture", "ALPHA BETA GAMMA DELTA")]);

    let first = results
        .query()
        .minimum_severity(Severity::High)
        .rule_id("beta")
        .collect();

    let second = results
        .query()
        .rule_id("beta")
        .minimum_severity(Severity::High)
        .collect();

    assert_eq!(first, second);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].1.rule_id().as_str(), "beta");
}

#[test]
fn later_severity_constraint_replaces_earlier_constraint() {
    let scanner = scanner();
    let results = scanner.scan([("fixture", "ALPHA BETA GAMMA")]);

    let exact_then_minimum = results
        .query()
        .severity(Severity::Critical)
        .minimum_severity(Severity::High)
        .collect();

    assert_eq!(exact_then_minimum.len(), 2);

    let minimum_then_exact = results
        .query()
        .minimum_severity(Severity::High)
        .severity(Severity::Critical)
        .collect();

    assert_eq!(minimum_then_exact.len(), 1);
    assert_eq!(minimum_then_exact[0].1.severity(), Severity::Critical,);
}

#[test]
fn empty_and_single_match_queries_have_consistent_terminal_helpers() {
    let scanner = scanner();
    let results = scanner.scan([("fixture", "ALPHA BETA")]);

    let empty = results.query().rule_id("missing");
    assert!(empty.is_empty());
    assert!(!empty.has_matches());
    assert_eq!(empty.count(), 0);
    assert!(empty.first().is_none());
    assert!(empty.last().is_none());
    assert!(empty.collect().is_empty());

    let single = results.query().rule_id("alpha");
    assert!(!single.is_empty());
    assert!(single.has_matches());
    assert_eq!(single.count(), 1);
    assert_eq!(single.first(), single.last());
}

#[test]
fn sorting_does_not_mutate_original_query_order() {
    let scanner = scanner();
    let results = scanner.scan([("z.env", "BETA"), ("a.env", "ALPHA"), ("m.env", "GAMMA")]);

    let original = results
        .query()
        .collect()
        .into_iter()
        .map(|(key, finding)| (*key, finding.rule_id().as_str()))
        .collect::<Vec<_>>();

    let sorted = results
        .query()
        .sort(ScanSort::RuleId)
        .into_vec()
        .into_iter()
        .map(|(key, finding)| (*key, finding.rule_id().as_str()))
        .collect::<Vec<_>>();

    let after = results
        .query()
        .collect()
        .into_iter()
        .map(|(key, finding)| (*key, finding.rule_id().as_str()))
        .collect::<Vec<_>>();

    assert_eq!(original, after);
    assert_ne!(original, sorted);
    assert_eq!(
        sorted,
        [("a.env", "alpha"), ("z.env", "beta"), ("m.env", "gamma"),],
    );
}

#[test]
fn ascending_and_descending_rule_sort_are_exact_reverses_without_ties() {
    let scanner = scanner();
    let results = scanner.scan([("fixture", "ALPHA BETA GAMMA DELTA")]);

    let ascending = results
        .query()
        .sort(ScanSort::RuleId)
        .iter()
        .map(|(_, finding)| finding.rule_id().as_str())
        .collect::<Vec<_>>();

    let descending = results
        .query()
        .sort(ScanSort::RuleIdDescending)
        .iter()
        .map(|(_, finding)| finding.rule_id().as_str())
        .collect::<Vec<_>>();

    let reversed = ascending.iter().rev().copied().collect::<Vec<_>>();
    assert_eq!(descending, reversed);
}

#[test]
fn source_sort_handles_duplicate_source_content_deterministically() {
    let scanner = scanner();
    let results = scanner.scan([("c.env", "ALPHA"), ("a.env", "ALPHA"), ("b.env", "ALPHA")]);

    let sorted = results
        .query()
        .sort(ScanSort::Source)
        .iter()
        .map(|(key, _)| *key)
        .collect::<Vec<_>>();

    assert_eq!(sorted, ["a.env", "b.env", "c.env"]);
}

#[test]
fn query_handles_large_finding_sets() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("hit", "X", Severity::High))
        .build()
        .expect("scanner should build");

    let source = "X ".repeat(10_000);
    let results = scanner.scan([("dense", source.as_str())]);
    let query = results.query().minimum_severity(Severity::High);

    assert_eq!(query.count(), 10_000);
    assert!(query.has_matches());

    let sorted = query.sort(ScanSort::Location);
    assert_eq!(sorted.len(), 10_000);
    assert_eq!(sorted.first().unwrap().1.location().start(), 0);
    assert!(sorted.last().unwrap().1.location().start() > 0);
}
