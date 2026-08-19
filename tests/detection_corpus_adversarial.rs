//! Adversarial extension of the Cribra v0.2 corpus.
//!
//! These tests freeze boundary behavior that tends to regress when detector
//! coverage grows: malformed provider tokens, punctuation/boundary handling,
//! opaque infrastructure identifiers, Unicode coordinates and mixed config
//! layouts.

use std::collections::BTreeSet;

use cribra::{Scanner, SensitiveCandidateKind};

const TRUNCATED: &str = include_str!("fixtures/corpus_v02/adversarial-truncated.txt");
const PUNCTUATION: &str = include_str!("fixtures/corpus_v02/adversarial-punctuation.txt");
const HARMLESS: &str = include_str!("fixtures/corpus_v02/adversarial-harmless.txt");
const MIXED_CONTEXT: &str = include_str!("fixtures/corpus_v02/adversarial-context.txt");

fn finding_ids(report: &cribra::ScanReport) -> BTreeSet<&str> {
    report
        .findings()
        .iter()
        .map(|finding| finding.rule_id().as_str())
        .collect()
}

#[test]
fn truncated_provider_shapes_are_not_promoted_when_isolated() {
    let scanner = Scanner::default();
    let results = scanner.scan([("truncated", TRUNCATED)]);
    let report = results.single_report().expect("one fixture was scanned");

    assert!(
        report.findings().is_empty(),
        "truncated isolated provider shapes unexpectedly produced findings: {:?}",
        finding_ids(report)
    );
    assert!(report.candidates().is_empty());
}

#[test]
fn recovery_like_boundary_rules_accept_punctuation_but_reject_embedding() {
    let scanner = Scanner::default();
    let results = scanner.scan([("punctuation", PUNCTUATION)]);
    let report = results.single_report().expect("one fixture was scanned");

    assert!(report.findings().is_empty());
    assert_eq!(report.candidate_len(), 2);

    let spans = report
        .candidates()
        .iter()
        .map(|candidate| {
            assert_eq!(candidate.kind(), SensitiveCandidateKind::RecoveryLikeCode);
            &PUNCTUATION[candidate.location().byte_range()]
        })
        .collect::<Vec<_>>();

    assert_eq!(spans, ["ABCD-EFGH-IJKL-MNOP", "QRST-UVWX-YZ12-3456"]);
}

#[test]
fn common_opaque_infrastructure_identifiers_remain_clean() {
    let scanner = Scanner::default();
    let results = scanner.scan([("harmless", HARMLESS)]);
    let report = results.single_report().expect("one fixture was scanned");

    assert!(
        report.findings().is_empty(),
        "harmless infrastructure corpus produced findings: {:?}",
        finding_ids(report)
    );
    assert!(
        report.candidates().is_empty(),
        "harmless infrastructure corpus produced review candidates"
    );
}

#[test]
fn mixed_config_layouts_preserve_contextual_detector_semantics() {
    let scanner = Scanner::default();
    let results = scanner.scan([("mixed-context", MIXED_CONTEXT)]);
    let report = results.single_report().expect("one fixture was scanned");
    let ids = finding_ids(report);

    for expected in [
        "generic.api-key",
        "generic.database-password-field",
        "generic.sensitive-hash",
        "azure.client-secret",
    ] {
        assert!(
            ids.contains(expected),
            "mixed contextual corpus missed {expected}; found {ids:?}"
        );
    }
}

#[test]
fn deterministic_finding_after_unicode_prefix_has_exact_byte_and_scalar_coordinates() {
    const TOKEN: &str = "ghp_AbCdEf0123456789_AbCdEf0123456789";
    let source = format!("😀 αβγ\nvalue={TOKEN}\n");

    let scanner = Scanner::default();
    let results = scanner.scan([("unicode", source.as_str())]);
    let report = results.single_report().expect("one fixture was scanned");

    let finding = report
        .findings()
        .iter()
        .find(|finding| finding.rule_id().as_str() == "github.classic-pat")
        .expect("synthetic GitHub token should be detected");

    assert_eq!(
        &source[finding.location().byte_range()],
        TOKEN,
        "finding span must contain only the credential"
    );
    assert_eq!(finding.location().line(), 2);
    assert_eq!(finding.location().column(), 7);
}

#[test]
fn nearby_classified_and_ambiguous_values_remain_separate_channels() {
    const TOKEN: &str = "ghp_AbCdEf0123456789_AbCdEf0123456789";
    const CANDIDATE: &str = "QRST-UVWX-YZ12-3456";
    let source = format!("{TOKEN},{CANDIDATE}");

    let scanner = Scanner::default();
    let results = scanner.scan([("nearby", source.as_str())]);
    let report = results.single_report().expect("one fixture was scanned");

    assert_eq!(
        report
            .findings()
            .iter()
            .filter(|finding| finding.rule_id().as_str() == "github.classic-pat")
            .count(),
        1
    );
    assert_eq!(report.candidate_len(), 1);
    assert_eq!(
        &source[report.candidates()[0].location().byte_range()],
        CANDIDATE
    );
}
