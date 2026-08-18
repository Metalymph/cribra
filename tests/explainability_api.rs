//! Serde and public-API regression contract for explainability.
//!
//! v0.2.4 keeps explanation facts computed rather than embedded into scan
//! results. This suite freezes the application boundary:
//!
//! - `Explanation` has a small, stable Serde representation;
//! - findings and sensitive candidates do not gain duplicate explanation state;
//! - explanation can be reconstructed from the public API after deserialization;
//! - classified and ambiguous explanation variants remain distinct;
//! - no source or matched sensitive value enters explanation payloads.

use silens_scan::{CandidateEvidence, DetectionMode, Explanation, Rule, Scanner, Severity};

const SECRET: &str = "KNOWN_SECRET_VALUE";
const AMBIGUOUS: &str = "ABCD-EFGH-IJKL-MNOP";

#[test]
fn explanation_public_helpers_are_consistent() {
    let classified = Explanation::classified(DetectionMode::Contextual);
    let ambiguous = Explanation::ambiguous(CandidateEvidence::Structural);

    assert_eq!(
        classified,
        Explanation::Classified(DetectionMode::Contextual)
    );
    assert_eq!(
        ambiguous,
        Explanation::Ambiguous(CandidateEvidence::Structural)
    );

    assert_eq!(classified.detection_mode(), Some(DetectionMode::Contextual));
    assert_eq!(classified.candidate_evidence(), None);
    assert!(classified.is_classified());
    assert!(!classified.is_ambiguous());

    assert_eq!(ambiguous.detection_mode(), None);
    assert_eq!(
        ambiguous.candidate_evidence(),
        Some(CandidateEvidence::Structural)
    );
    assert!(!ambiguous.is_classified());
    assert!(ambiguous.is_ambiguous());
}

#[test]
fn explanations_are_computed_not_stored_in_scan_results() {
    let scanner = Scanner::builder()
        .rule(Rule::literal(
            "organization.known-secret",
            SECRET,
            Severity::High,
        ))
        .build()
        .expect("scanner should compile");

    let results = scanner.scan([("known-source", SECRET), ("review-source", AMBIGUOUS)]);

    let classified = results
        .iter()
        .find(|entry| entry.key() == &"known-source")
        .expect("classified entry should exist");
    let ambiguous = results
        .iter()
        .find(|entry| entry.key() == &"review-source")
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

#[cfg(feature = "serde")]
mod serde_contract {
    use super::*;

    #[test]
    fn classified_json_shape_is_stable() {
        let explanation = Explanation::Classified(DetectionMode::Deterministic);

        assert_eq!(
            serde_json::to_string(&explanation).expect("serialize explanation"),
            r#"{"classified":"deterministic"}"#
        );
    }

    #[test]
    fn ambiguous_json_shape_is_stable() {
        let explanation = Explanation::Ambiguous(CandidateEvidence::Structural);

        assert_eq!(
            serde_json::to_string(&explanation).expect("serialize explanation"),
            r#"{"ambiguous":"structural"}"#
        );
    }

    #[test]
    fn explanation_round_trip_preserves_both_variants() {
        for explanation in [
            Explanation::Classified(DetectionMode::MatcherOnly),
            Explanation::Classified(DetectionMode::Deterministic),
            Explanation::Classified(DetectionMode::Contextual),
            Explanation::Ambiguous(CandidateEvidence::Structural),
        ] {
            let json = serde_json::to_string(&explanation).expect("serialize explanation");
            let decoded: Explanation =
                serde_json::from_str(&json).expect("deserialize explanation");

            assert_eq!(decoded, explanation);
        }
    }

    #[test]
    fn scan_result_json_does_not_embed_computed_explanation_state() {
        let scanner = Scanner::builder()
            .rule(Rule::literal(
                "organization.known-secret",
                SECRET,
                Severity::High,
            ))
            .build()
            .expect("scanner should compile");

        let results = scanner.scan([("classified", SECRET), ("ambiguous", AMBIGUOUS)]);

        let json = serde_json::to_string(&results).expect("serialize scan results");

        assert!(!json.contains("\"explanation\""));
        assert!(!json.contains("\"matcher_only\""));
        assert!(!json.contains("\"classified\":"));
        assert!(!json.contains("\"ambiguous\":\"structural\""));

        // Existing result authorities remain serialized normally.
        assert!(json.contains("\"findings\""));
        assert!(json.contains("\"candidates\""));
        assert!(json.contains("\"evidence\":\"structural\""));

        // Public result serialization remains metadata-only.
        assert!(!json.contains(SECRET));
        assert!(!json.contains(AMBIGUOUS));
    }

    #[test]
    fn candidate_explanation_is_reconstructible_after_candidate_deserialization() {
        let scanner = Scanner::default();
        let results = scanner.scan([("ambiguous", AMBIGUOUS)]);
        let candidate = &results
            .single_report()
            .expect("one source was scanned")
            .candidates()[0];

        let json = serde_json::to_string(candidate).expect("serialize candidate");
        let decoded: silens_scan::SensitiveCandidate =
            serde_json::from_str(&json).expect("deserialize candidate");

        assert_eq!(
            decoded.explanation(),
            Explanation::Ambiguous(CandidateEvidence::Structural)
        );
    }

    #[test]
    fn finding_explanation_remains_scanner_authoritative_after_deserialization() {
        let scanner = Scanner::builder()
            .rule(Rule::literal(
                "organization.known-secret",
                SECRET,
                Severity::High,
            ))
            .build()
            .expect("scanner should compile");

        let results = scanner.scan([("classified", SECRET)]);
        let finding = &results
            .single_report()
            .expect("one source was scanned")
            .findings()[0];

        let json = serde_json::to_string(finding).expect("serialize finding");
        let decoded: silens_scan::Finding =
            serde_json::from_str(&json).expect("deserialize finding");

        assert_eq!(
            decoded.explanation(&scanner),
            Some(Explanation::Classified(DetectionMode::MatcherOnly))
        );

        let unrelated = Scanner::builder()
            .build()
            .expect("empty scanner should compile");

        assert_eq!(decoded.explanation(&unrelated), None);
    }

    #[test]
    fn explanation_payloads_never_contain_source_values() {
        let classified =
            serde_json::to_string(&Explanation::Classified(DetectionMode::MatcherOnly))
                .expect("serialize classified explanation");
        let ambiguous =
            serde_json::to_string(&Explanation::Ambiguous(CandidateEvidence::Structural))
                .expect("serialize ambiguous explanation");

        for payload in [classified, ambiguous] {
            assert!(!payload.contains(SECRET));
            assert!(!payload.contains(AMBIGUOUS));
            assert!(!payload.contains("matched_value"));
            assert!(!payload.contains("source"));
        }
    }
}
