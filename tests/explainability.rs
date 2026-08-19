//! Public regression contract for typed explainability.
//!
//! This suite freezes the v0.2.4 boundary between classified findings and
//! ambiguous review candidates.
//!
//! Explainability is factual and presentation-agnostic:
//!
//! - findings resolve to `Explanation::Classified(DetectionMode)` through the
//!   scanner's rule metadata authority;
//! - sensitive candidates resolve to
//!   `Explanation::Ambiguous(CandidateEvidence)` directly from candidate
//!   evidence;
//! - neither path exposes source text or matched sensitive values;
//! - ambiguity never acquires finding severity, confidence or remediation.

use cribra::{
    CandidateEvidence, DetectionMode, Explanation, Rule, Scanner, SensitiveCandidateKind, Severity,
};

const AMBIGUOUS: &str = "ABCD-EFGH-IJKL-MNOP";
const CLASSIFIED: &str = "KNOWN_SECRET_VALUE";

#[test]
fn classified_and_ambiguous_results_use_distinct_explanation_variants() {
    let scanner = Scanner::builder()
        .rule(Rule::literal(
            "organization.known-secret",
            CLASSIFIED,
            Severity::High,
        ))
        .build()
        .expect("scanner should compile");

    let results = scanner.scan([("classified", CLASSIFIED), ("ambiguous", AMBIGUOUS)]);

    let classified = results
        .iter()
        .find(|entry| entry.key() == &"classified")
        .expect("classified entry should exist");
    let ambiguous = results
        .iter()
        .find(|entry| entry.key() == &"ambiguous")
        .expect("ambiguous entry should exist");

    let finding = &classified.report().findings()[0];
    let candidate = &ambiguous.report().candidates()[0];

    assert_eq!(
        finding.explanation(&scanner),
        Some(Explanation::Classified(DetectionMode::MatcherOnly))
    );
    assert_eq!(
        candidate.explanation(),
        Explanation::Ambiguous(CandidateEvidence::Structural)
    );
}

#[test]
fn candidate_explanation_is_derived_from_candidate_evidence() {
    let scanner = Scanner::default();
    let results = scanner.scan([("ambiguous", AMBIGUOUS)]);
    let report = results.single_report().expect("one source was scanned");

    assert_eq!(report.findings().len(), 0);
    assert_eq!(report.candidate_len(), 1);

    let candidate = &report.candidates()[0];

    assert_eq!(candidate.kind(), SensitiveCandidateKind::RecoveryLikeCode);
    assert_eq!(candidate.evidence(), CandidateEvidence::Structural);
    assert_eq!(
        candidate.explanation(),
        Explanation::ambiguous(candidate.evidence())
    );
    assert_eq!(candidate.explanation().detection_mode(), None);
    assert_eq!(
        candidate.explanation().candidate_evidence(),
        Some(CandidateEvidence::Structural)
    );
    assert!(candidate.explanation().is_ambiguous());
    assert!(!candidate.explanation().is_classified());
}

#[test]
fn finding_explanation_is_derived_from_scanner_rule_authority() {
    let scanner = Scanner::builder()
        .rule(Rule::literal(
            "organization.known-secret",
            CLASSIFIED,
            Severity::High,
        ))
        .build()
        .expect("scanner should compile");

    let results = scanner.scan([("classified", CLASSIFIED)]);
    let report = results.single_report().expect("one source was scanned");
    let finding = &report.findings()[0];

    let explanation = finding
        .explanation(&scanner)
        .expect("finding should resolve against producing scanner");

    assert_eq!(
        explanation,
        Explanation::Classified(DetectionMode::MatcherOnly)
    );
    assert_eq!(
        explanation.detection_mode(),
        Some(DetectionMode::MatcherOnly)
    );
    assert_eq!(explanation.candidate_evidence(), None);
    assert!(explanation.is_classified());
    assert!(!explanation.is_ambiguous());
}

#[test]
fn candidate_explanation_does_not_require_scanner_lookup() {
    let scanner = Scanner::default();
    let results = scanner.scan([("ambiguous", AMBIGUOUS)]);
    let report = results.single_report().expect("one source was scanned");
    let candidate = &report.candidates()[0];

    // Candidate evidence is already part of the candidate authority. Unlike a
    // Finding, no scanner lookup is required and no rule metadata is invented.
    assert_eq!(
        candidate.explanation(),
        Explanation::Ambiguous(CandidateEvidence::Structural)
    );
}

#[test]
fn confirmed_finding_suppresses_ambiguous_explanation_for_same_span() {
    let scanner = Scanner::builder()
        .rule(Rule::literal(
            "organization.recovery-code",
            AMBIGUOUS,
            Severity::Critical,
        ))
        .build()
        .expect("scanner should compile");

    let results = scanner.scan([("known", AMBIGUOUS)]);
    let report = results.single_report().expect("one source was scanned");

    assert_eq!(report.findings().len(), 1);
    assert_eq!(report.candidate_len(), 0);

    assert_eq!(
        report.findings()[0].explanation(&scanner),
        Some(Explanation::Classified(DetectionMode::MatcherOnly))
    );
}

#[test]
fn unrelated_scanner_cannot_explain_a_classified_finding() {
    let producing_scanner = Scanner::builder()
        .rule(Rule::literal(
            "organization.known-secret",
            CLASSIFIED,
            Severity::High,
        ))
        .build()
        .expect("scanner should compile");

    let unrelated_scanner = Scanner::builder()
        .build()
        .expect("empty scanner should compile");

    let results = producing_scanner.scan([("classified", CLASSIFIED)]);
    let report = results.single_report().expect("one source was scanned");
    let finding = &report.findings()[0];

    assert_eq!(finding.explanation(&unrelated_scanner), None);
}

#[cfg(feature = "serde")]
#[test]
fn explanation_serialization_contains_only_typed_facts() {
    let scanner = Scanner::builder()
        .rule(Rule::literal(
            "organization.known-secret",
            CLASSIFIED,
            Severity::High,
        ))
        .build()
        .expect("scanner should compile");

    let classified_results = scanner.scan([("classified", CLASSIFIED)]);
    let finding = &classified_results
        .single_report()
        .expect("one source was scanned")
        .findings()[0];
    let classified = finding
        .explanation(&scanner)
        .expect("finding should resolve against producing scanner");

    let ambiguous_results = Scanner::default().scan([("ambiguous", AMBIGUOUS)]);
    let candidate = &ambiguous_results
        .single_report()
        .expect("one source was scanned")
        .candidates()[0];
    let ambiguous = candidate.explanation();

    let classified_json =
        serde_json::to_string(&classified).expect("serialize classified explanation");
    let ambiguous_json =
        serde_json::to_string(&ambiguous).expect("serialize ambiguous explanation");

    assert!(classified_json.contains("classified"));
    assert!(classified_json.contains("matcher_only"));
    assert!(ambiguous_json.contains("ambiguous"));
    assert!(ambiguous_json.contains("structural"));

    for json in [&classified_json, &ambiguous_json] {
        assert!(!json.contains(CLASSIFIED));
        assert!(!json.contains(AMBIGUOUS));
        assert!(!json.contains("matched_value"));
        assert!(!json.contains("source"));
    }
}
