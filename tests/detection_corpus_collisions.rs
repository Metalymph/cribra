//! Collision, overlap and multi-detector regression contract for Cribra v0.2.

use cribra::{Rule, Scanner, Severity, builtins};

const TOKEN: &str = "ghp_AbCdEf0123456789_AbCdEf0123456789";
const CANDIDATE: &str = "QRST-UVWX-YZ12-3456";

#[test]
fn equal_priority_identical_custom_spans_are_ordered_by_rule_id_not_insertion_order() {
    fn ids(reverse: bool) -> Vec<String> {
        let first = Rule::literal("zeta.rule", "shared-secret", Severity::High);
        let second = Rule::literal("alpha.rule", "shared-secret", Severity::High);

        let scanner = if reverse {
            Scanner::builder().rule(second).rule(first)
        } else {
            Scanner::builder().rule(first).rule(second)
        }
        .build()
        .expect("scanner should compile");

        scanner
            .scan([("source", "shared-secret")])
            .single_report()
            .expect("one source")
            .findings()
            .iter()
            .map(|finding| finding.rule_id().as_str().to_owned())
            .collect()
    }

    assert_eq!(ids(false), ["alpha.rule", "zeta.rule"]);
    assert_eq!(ids(false), ids(true));
}

#[test]
fn partial_overlaps_remain_independently_observable_and_position_ordered() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("whole", "secret-value", Severity::High))
        .rule(Rule::literal("prefix", "secret", Severity::Medium))
        .rule(Rule::literal("suffix", "value", Severity::Low))
        .build()
        .expect("scanner should compile");

    let results = scanner.scan([("source", "secret-value")]);
    let report = results.single_report().expect("one source");
    let spans = report
        .findings()
        .iter()
        .map(|finding| {
            (
                finding.rule_id().as_str(),
                finding.location().start(),
                finding.location().end(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        spans,
        [("prefix", 0, 6), ("whole", 0, 12), ("suffix", 7, 12),]
    );
}

#[test]
fn custom_finding_promotes_ambiguous_span_and_suppresses_candidate() {
    let scanner = Scanner::builder()
        .builtins(builtins::CURRENT)
        .rule(Rule::literal(
            "domain.recovery-code",
            CANDIDATE,
            Severity::Critical,
        ))
        .build()
        .expect("scanner should compile");

    let results = scanner.scan([("source", CANDIDATE)]);
    let report = results.single_report().expect("one source");

    assert_eq!(report.findings().len(), 1);
    assert_eq!(
        report.findings()[0].rule_id().as_str(),
        "domain.recovery-code"
    );
    assert!(report.candidates().is_empty());
}

#[test]
fn nearby_non_overlapping_finding_and_candidate_remain_separate() {
    let source = format!("{TOKEN},{CANDIDATE}");
    let scanner = Scanner::default();
    let results = scanner.scan([("source", source.as_str())]);
    let report = results.single_report().expect("one source");

    assert!(
        report
            .findings()
            .iter()
            .any(|finding| finding.rule_id().as_str() == "github.classic-pat")
    );
    assert_eq!(report.candidate_len(), 1);
    assert_eq!(
        &source[report.candidates()[0].location().byte_range()],
        CANDIDATE
    );
}

#[test]
fn multiple_findings_on_one_line_are_sorted_by_source_position() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("third", "THREE", Severity::Low))
        .rule(Rule::literal("first", "ONE", Severity::Critical))
        .rule(Rule::literal("second", "TWO", Severity::High))
        .build()
        .expect("scanner should compile");

    let results = scanner.scan([("source", "ONE TWO THREE")]);
    let report = results.single_report().expect("one source");

    let ids = report
        .findings()
        .iter()
        .map(|finding| finding.rule_id().as_str())
        .collect::<Vec<_>>();

    assert_eq!(ids, ["first", "second", "third"]);
}

#[cfg(feature = "parallel")]
#[test]
fn collision_heavy_batch_is_serial_parallel_equivalent() {
    let scanner = Scanner::builder()
        .builtins(builtins::CURRENT)
        .rule(Rule::literal(
            "domain.recovery-code",
            CANDIDATE,
            Severity::Critical,
        ))
        .rule(Rule::literal(
            "overlap.whole",
            "secret-value",
            Severity::High,
        ))
        .rule(Rule::literal("overlap.part", "secret", Severity::Medium))
        .build()
        .expect("scanner should compile");

    let sources = [
        ("provider", TOKEN),
        ("promoted", CANDIDATE),
        ("overlap", "secret-value"),
        ("mixed", "ordinary text"),
    ];

    let serial = scanner.scan(sources);
    let parallel = scanner.parallel_scan(sources);

    assert_eq!(serial, parallel);
}
