//! Cribra v0.2 detection-corpus regression contract.
//!
//! This suite complements the focused built-in/contextual/ambiguity tests with
//! a corpus-level boundary. It deliberately tests behavior, not implementation:
//!
//! - representative deterministic and contextual credentials are found;
//! - isolated contextual values are not promoted without semantic context;
//! - ambiguous recovery-like shapes remain review-only;
//! - common opaque identifiers/checksums/package-integrity values remain clean;
//! - findings and candidates never silently collapse into one result channel.
//!
//! Every credential-looking value in this corpus is synthetic and intentionally
//! unusable.

use std::collections::BTreeSet;

use cribra::{CandidateEvidence, Scanner, SensitiveCandidateKind};

const POSITIVE: &str = include_str!("fixtures/corpus_v02/positive.env");
const FALSE_POSITIVES: &str = include_str!("fixtures/corpus_v02/false-positives.txt");
const AMBIGUOUS: &str = include_str!("fixtures/corpus_v02/ambiguous.txt");
const CONTEXTUAL_ISOLATED: &str = include_str!("fixtures/corpus_v02/contextual-isolated.txt");

fn ids(report: &cribra::ScanReport) -> BTreeSet<&str> {
    report
        .findings()
        .iter()
        .map(|finding| finding.rule_id().as_str())
        .collect()
}

#[test]
fn representative_positive_corpus_detects_expected_families() {
    let scanner = Scanner::default();
    let results = scanner.scan([("positive", POSITIVE)]);
    let report = results
        .single_report()
        .expect("one corpus file was scanned");
    let found = ids(report);

    for expected in [
        "github.classic-pat",
        "stripe.live-secret-key",
        "aws.secret-access-key",
        "azure.client-secret",
        "generic.database-password-field",
        "generic.api-key",
        "generic.sensitive-hash",
    ] {
        assert!(
            found.contains(expected),
            "positive corpus missed expected rule {expected}; found {found:?}"
        );
    }

    assert!(
        report.candidates().is_empty(),
        "classified positive corpus should not leak into the ambiguous channel"
    );
}

#[test]
fn isolated_contextual_values_remain_unclassified() {
    let scanner = Scanner::default();
    let results = scanner.scan([("isolated-contextual", CONTEXTUAL_ISOLATED)]);
    let report = results
        .single_report()
        .expect("one corpus file was scanned");

    assert!(
        report.findings().is_empty(),
        "isolated contextual values unexpectedly became findings: {:?}",
        ids(report)
    );
}

#[test]
fn ambiguous_corpus_is_review_only() {
    let scanner = Scanner::default();
    let results = scanner.scan([("ambiguous", AMBIGUOUS)]);
    let report = results
        .single_report()
        .expect("one corpus file was scanned");

    assert!(report.findings().is_empty());
    assert_eq!(report.candidate_len(), 4);

    for candidate in report.candidates() {
        assert_eq!(candidate.kind(), SensitiveCandidateKind::RecoveryLikeCode);
        assert_eq!(candidate.evidence(), CandidateEvidence::Structural);
    }

    assert!(report.needs_review());
}

#[test]
fn false_positive_corpus_remains_clean() {
    let scanner = Scanner::default();
    let results = scanner.scan([("false-positives", FALSE_POSITIVES)]);
    let report = results
        .single_report()
        .expect("one corpus file was scanned");

    assert!(
        report.findings().is_empty(),
        "false-positive corpus produced findings: {:?}",
        report
            .findings()
            .iter()
            .map(|finding| finding.rule_id().as_str())
            .collect::<Vec<_>>()
    );

    assert!(
        report.candidates().is_empty(),
        "false-positive corpus produced candidates: {:?}",
        report
            .candidates()
            .iter()
            .map(|candidate| {
                (
                    candidate.kind(),
                    &FALSE_POSITIVES[candidate.location().byte_range()],
                )
            })
            .collect::<Vec<_>>()
    );

    assert!(!report.needs_review());
}

#[test]
fn batch_classification_keeps_failed_review_and_clean_sources_distinct() {
    let scanner = Scanner::default();
    let results = scanner.scan([
        ("positive", POSITIVE),
        ("ambiguous", AMBIGUOUS),
        ("false-positive", FALSE_POSITIVES),
    ]);

    assert_eq!(results.failed().count(), 1);
    assert_eq!(results.review().count(), 1);
    assert_eq!(results.clean().count(), 1);
    assert!(results.total_findings() >= 7);
    assert_eq!(results.total_candidates(), 4);
}
