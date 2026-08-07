//! Hardening tests for serial/parallel semantic equivalence.

#![cfg(feature = "parallel")]

use silens_scan::{Rule, ScanSort, Scanner, Severity};

fn scanner() -> Scanner {
    Scanner::builder()
        .rule(Rule::literal("critical", "CRITICAL", Severity::Critical))
        .rule(Rule::literal("high", "HIGH", Severity::High))
        .rule(Rule::literal("medium", "MEDIUM", Severity::Medium))
        .build()
        .expect("parallel hardening scanner should build")
}

#[test]
fn empty_batches_are_identical() {
    let scanner = scanner();

    let serial = scanner.scan(std::iter::empty::<(&str, &str)>());
    let parallel = scanner.parallel_scan(std::iter::empty::<(&str, &str)>());

    assert_eq!(serial, parallel);
    assert!(serial.is_empty());
    assert_eq!(serial.summary(), parallel.summary());
}

#[test]
fn one_element_batch_is_identical() {
    let scanner = scanner();

    let serial = scanner.scan([("one", "CRITICAL HIGH")]);
    let parallel = scanner.parallel_scan([("one", "CRITICAL HIGH")]);

    assert_eq!(serial, parallel);
    assert_eq!(serial.summary(), parallel.summary());
    assert_eq!(
        serial.query().sort(ScanSort::RuleId).into_vec(),
        parallel.query().sort(ScanSort::RuleId).into_vec(),
    );
}

#[test]
fn large_parallel_batch_preserves_order_and_reports() {
    let scanner = scanner();

    let sources = (0..512)
        .map(|index| {
            let source = match index % 4 {
                0 => "CRITICAL HIGH MEDIUM",
                1 => "HIGH",
                2 => "MEDIUM MEDIUM",
                _ => "clean",
            };
            (index, source)
        })
        .collect::<Vec<_>>();

    let serial = scanner.scan(sources.iter().copied());
    let parallel = scanner.parallel_scan(sources.iter().copied());

    assert_eq!(serial, parallel);
    assert_eq!(serial.len(), 512);

    let keys = parallel
        .iter()
        .map(|entry| *entry.key())
        .collect::<Vec<_>>();

    assert_eq!(keys, (0..512).collect::<Vec<_>>());
}

#[test]
fn serial_and_parallel_summaries_are_identical() {
    let scanner = scanner();
    let inputs = [
        ("a", "CRITICAL"),
        ("b", "HIGH HIGH"),
        ("c", "MEDIUM"),
        ("d", "clean"),
        ("e", "CRITICAL HIGH MEDIUM"),
    ];

    let serial = scanner.scan(inputs);
    let parallel = scanner.parallel_scan(inputs);

    assert_eq!(serial.summary(), parallel.summary());
    assert_eq!(serial.total_findings(), parallel.total_findings());
    assert_eq!(serial.total_bytes(), parallel.total_bytes());
    assert_eq!(serial.has_critical(), parallel.has_critical());
    assert_eq!(serial.clean().count(), parallel.clean().count());
    assert_eq!(serial.failed().count(), parallel.failed().count());
}

#[test]
fn serial_and_parallel_query_results_are_identical() {
    let scanner = scanner();
    let inputs = [
        ("z", "HIGH"),
        ("a", "CRITICAL HIGH"),
        ("m", "MEDIUM"),
        ("b", "CRITICAL"),
    ];

    let serial = scanner.scan(inputs);
    let parallel = scanner.parallel_scan(inputs);

    let serial_query = serial
        .query()
        .minimum_severity(Severity::High)
        .sort(ScanSort::RuleId)
        .into_vec();

    let parallel_query = parallel
        .query()
        .minimum_severity(Severity::High)
        .sort(ScanSort::RuleId)
        .into_vec();

    assert_eq!(serial_query, parallel_query);
}
