//! Public regression contract for ambiguous sensitive candidates.
//!
//! This suite freezes the v0.2.3 boundary:
//!
//! - structurally plausible isolated recovery-like values become candidates;
//! - candidates never become findings merely because their structure is unusual;
//! - confirmed findings suppress overlapping ambiguous candidates;
//! - known false-positive families remain outside the candidate channel;
//! - transformations continue to consume findings only;
//! - serialized candidates expose metadata, never the source value.

use silens_scan::{
    CandidateEvidence, Rule, Scanner, SensitiveCandidateKind, Severity, transform::redact,
};

const AMBIGUOUS: &str = include_str!("fixtures/ambiguous-recovery-like.txt");
const FALSE_POSITIVES: &str = include_str!("fixtures/ambiguous-false-positives.txt");
const ISOLATED: &str = "ABCD-EFGH-IJKL-MNOP";

#[test]
fn isolated_recovery_like_value_is_review_only() {
    let scanner = Scanner::default();
    let results = scanner.scan([("isolated", ISOLATED)]);
    let report = results.single_report().expect("one source was scanned");

    assert_eq!(report.len(), 0);
    assert!(report.is_empty());
    assert_eq!(report.candidate_len(), 1);
    assert!(report.has_candidates());
    assert!(report.needs_review());

    let candidate = &report.candidates()[0];

    assert_eq!(candidate.kind(), SensitiveCandidateKind::RecoveryLikeCode);
    assert_eq!(candidate.evidence(), CandidateEvidence::Structural);
    assert_eq!(&ISOLATED[candidate.location().byte_range()], ISOLATED);
}

#[test]
fn ambiguous_fixture_produces_candidates_without_findings() {
    let scanner = Scanner::default();
    let results = scanner.scan([("ambiguous", AMBIGUOUS)]);
    let report = results.single_report().expect("one source was scanned");

    assert_eq!(report.len(), 0);
    assert_eq!(report.candidate_len(), 4);
    assert_eq!(results.total_findings(), 0);
    assert_eq!(results.total_candidates(), 4);
    assert_eq!(results.findings().count(), 0);
    assert_eq!(results.candidates().count(), 4);
    assert_eq!(results.review().count(), 1);
    assert_eq!(results.clean().count(), 0);

    for candidate in report.candidates() {
        assert_eq!(candidate.kind(), SensitiveCandidateKind::RecoveryLikeCode);
        assert_eq!(candidate.evidence(), CandidateEvidence::Structural);
    }
}

#[test]
fn ambiguous_candidates_preserve_source_order_and_coordinates() {
    let scanner = Scanner::default();
    let results = scanner.scan([("ambiguous", AMBIGUOUS)]);
    let report = results.single_report().expect("one source was scanned");

    let spans = report
        .candidates()
        .iter()
        .map(|candidate| {
            (
                &AMBIGUOUS[candidate.location().byte_range()],
                candidate.location().line(),
                candidate.location().column(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        spans,
        vec![
            ("ABCD-EFGH-IJKL-MNOP", 2, 1),
            ("QRST-UVWX-YZ12-3456", 3, 1),
            ("ZXCV-BNMQ-WER7-TYUI", 4, 9),
            ("ASDF-GHJK-LMNP-QWER", 5, 18),
        ]
    );
}

#[test]
fn false_positive_fixture_has_no_findings_or_candidates() {
    let scanner = Scanner::default();
    let results = scanner.scan([("false-positive", FALSE_POSITIVES)]);
    let report = results.single_report().expect("one source was scanned");

    assert!(report.findings().is_empty());
    assert!(report.candidates().is_empty());
    assert!(!report.needs_review());
    assert_eq!(results.total_findings(), 0);
    assert_eq!(results.total_candidates(), 0);
    assert_eq!(results.clean().count(), 1);
    assert_eq!(results.review().count(), 0);
}

#[test]
fn explicit_rule_promotes_same_span_to_finding() {
    let scanner = Scanner::builder()
        .rule(Rule::literal(
            "organization.recovery-code",
            ISOLATED,
            Severity::Critical,
        ))
        .build()
        .expect("custom rule should compile");

    let results = scanner.scan([("known", ISOLATED)]);
    let report = results.single_report().expect("one source was scanned");

    assert_eq!(report.len(), 1);
    assert_eq!(
        report.findings()[0].rule_id().as_str(),
        "organization.recovery-code"
    );
    assert_eq!(report.candidate_len(), 0);
    assert!(!report.has_candidates());
}

#[test]
fn candidate_only_report_is_not_transformed_automatically() {
    let scanner = Scanner::default();
    let results = scanner.scan([("isolated", ISOLATED)]);
    let report = results.single_report().expect("one source was scanned");

    assert_eq!(report.len(), 0);
    assert_eq!(report.candidate_len(), 1);

    let transformed = redact(ISOLATED, report).expect("candidate-only redaction should be valid");

    assert_eq!(transformed, ISOLATED);
}

#[test]
fn summary_distinguishes_review_findings_and_clear_sources() {
    let scanner = Scanner::default();
    let results = scanner.scan([
        ("review", ISOLATED),
        ("clean", "ordinary configuration text"),
    ]);

    let summary = results.summary();

    assert_eq!(summary.scanned_sources(), 2);
    assert_eq!(summary.total_findings(), 0);
    assert_eq!(summary.total_candidates(), 1);
    assert_eq!(summary.reports_with_candidates(), 1);
    assert!(summary.has_candidates());
    assert!(!summary.is_clean());

    assert_eq!(results.review().count(), 1);
    assert_eq!(results.clean().count(), 1);
    assert_eq!(results.failed().count(), 0);
}

#[cfg(feature = "serde")]
#[test]
fn serialized_candidate_report_contains_metadata_not_source_value() {
    let scanner = Scanner::default();
    let results = scanner.scan([("isolated", ISOLATED)]);

    let json = serde_json::to_string(&results).expect("serialize scan results");

    assert!(json.contains("\"candidates\""));
    assert!(json.contains("\"recovery_like_code\""));
    assert!(json.contains("\"structural\""));
    assert!(!json.contains(ISOLATED));
}
