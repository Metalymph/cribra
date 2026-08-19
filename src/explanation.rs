//! Typed, presentation-agnostic explanation facts.
//!
//! Explainability in Cribra projects existing detection authorities into a
//! small public contract. It does not introduce a second classification system,
//! human-facing copy, source snippets, or matched sensitive values.

use crate::{CandidateEvidence, DetectionMode, Finding, Scanner};

/// Explains which existing authority caused a scan result to exist.
///
/// `Explanation` deliberately reuses [`DetectionMode`] for classified rule
/// results and [`CandidateEvidence`] for ambiguous review candidates. Consumers
/// can translate these facts into their own UI, protocol, or documentation
/// without the core owning presentation copy.
///
/// This enum does not contain source text or matched values.
#[non_exhaustive]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Explanation {
    /// The scanner classified a rule-backed result using the given detection mode.
    Classified(DetectionMode),

    /// The scanner surfaced a review-only candidate from the given evidence.
    Ambiguous(CandidateEvidence),
}

impl Explanation {
    /// Returns explanation facts for a classified rule-backed result.
    #[must_use]
    pub const fn classified(mode: DetectionMode) -> Self {
        Self::Classified(mode)
    }

    /// Returns explanation facts for an ambiguous review-only candidate.
    #[must_use]
    pub const fn ambiguous(evidence: CandidateEvidence) -> Self {
        Self::Ambiguous(evidence)
    }

    /// Resolves explanation facts for `finding` against `scanner`.
    ///
    /// Findings deliberately do not duplicate rule metadata. Rule identifiers
    /// are unique within a compiled scanner, so resolution uses the finding's
    /// stable rule identity and verifies the finding metadata against the
    /// scanner-owned authority.
    ///
    /// `None` is returned when the scanner does not contain metadata compatible
    /// with the finding. This covers deserialized findings resolved against an
    /// unrelated scanner without weakening the scanner-owned authority.
    ///
    /// Resolution performs no allocation and is outside the scanning hot path.
    /// It never inspects source text or matched values.
    #[must_use]
    pub fn for_finding(scanner: &Scanner, finding: &Finding) -> Option<Self> {
        scanner
            .rule_metadata()
            .find(|metadata| {
                metadata.id() == finding.rule_id().as_str()
                    && metadata.severity() == finding.severity()
                    && metadata.remediation() == finding.remediation()
            })
            .map(|metadata| metadata.explanation())
    }

    /// Returns the classified detection mode, when this explanation represents
    /// a rule-backed finding.
    #[must_use]
    pub const fn detection_mode(self) -> Option<DetectionMode> {
        match self {
            Self::Classified(mode) => Some(mode),
            Self::Ambiguous(_) => None,
        }
    }

    /// Returns candidate evidence when this explanation represents an ambiguous
    /// review-only value.
    #[must_use]
    pub const fn candidate_evidence(self) -> Option<CandidateEvidence> {
        match self {
            Self::Classified(_) => None,
            Self::Ambiguous(evidence) => Some(evidence),
        }
    }

    /// Returns `true` when the explanation describes a classified rule result.
    #[must_use]
    pub const fn is_classified(self) -> bool {
        matches!(self, Self::Classified(_))
    }

    /// Returns `true` when the explanation describes an ambiguous review item.
    #[must_use]
    pub const fn is_ambiguous(self) -> bool {
        matches!(self, Self::Ambiguous(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Confidence, Location, Remediation, Rule, RuleId, Severity};

    fn finding(rule_id: &str, severity: Severity, remediation: Option<Remediation>) -> Finding {
        Finding::new(
            RuleId::from(rule_id),
            Location::from_span(0, 6),
            severity,
            Confidence::High,
            remediation,
        )
    }

    #[test]
    fn classified_explanation_reuses_detection_mode() {
        let explanation = Explanation::classified(DetectionMode::Contextual);

        assert_eq!(
            explanation,
            Explanation::Classified(DetectionMode::Contextual)
        );
        assert_eq!(
            explanation.detection_mode(),
            Some(DetectionMode::Contextual)
        );
        assert_eq!(explanation.candidate_evidence(), None);
        assert!(explanation.is_classified());
        assert!(!explanation.is_ambiguous());
    }

    #[test]
    fn ambiguous_explanation_reuses_candidate_evidence() {
        let explanation = Explanation::ambiguous(CandidateEvidence::Structural);

        assert_eq!(
            explanation,
            Explanation::Ambiguous(CandidateEvidence::Structural)
        );
        assert_eq!(explanation.detection_mode(), None);
        assert_eq!(
            explanation.candidate_evidence(),
            Some(CandidateEvidence::Structural)
        );
        assert!(!explanation.is_classified());
        assert!(explanation.is_ambiguous());
    }

    #[test]
    fn finding_resolution_reuses_scanner_rule_metadata() {
        let scanner = Scanner::builder()
            .rule(Rule::literal("custom", "secret", Severity::High))
            .build()
            .expect("scanner should compile");
        let report = scanner.scan([("memory", "secret")]);
        let finding = &report
            .single_report()
            .expect("one source was scanned")
            .findings()[0];

        assert_eq!(
            Explanation::for_finding(&scanner, finding),
            Some(Explanation::Classified(DetectionMode::MatcherOnly))
        );
    }

    #[test]
    fn finding_resolution_fails_closed_for_unrelated_scanner() {
        let scanner = Scanner::builder().build().expect("scanner should compile");
        let finding = finding("missing", Severity::High, None);

        assert_eq!(Explanation::for_finding(&scanner, &finding), None);
    }

    #[test]
    fn finding_resolution_rejects_incompatible_metadata() {
        let scanner = Scanner::builder()
            .rule(Rule::literal("shared", "secret", Severity::Critical))
            .build()
            .expect("scanner should compile");
        let finding = finding("shared", Severity::High, None);

        assert_eq!(Explanation::for_finding(&scanner, &finding), None);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_contract_contains_only_typed_facts() {
        let classified = Explanation::classified(DetectionMode::Deterministic);
        let ambiguous = Explanation::ambiguous(CandidateEvidence::Structural);

        let classified_json =
            serde_json::to_string(&classified).expect("serialize classified explanation");
        let ambiguous_json =
            serde_json::to_string(&ambiguous).expect("serialize ambiguous explanation");
        let classified_round_trip: Explanation =
            serde_json::from_str(&classified_json).expect("deserialize classified explanation");
        let ambiguous_round_trip: Explanation =
            serde_json::from_str(&ambiguous_json).expect("deserialize ambiguous explanation");

        assert_eq!(classified_round_trip, classified);
        assert_eq!(ambiguous_round_trip, ambiguous);
        assert!(!classified_json.contains("matched_value"));
        assert!(!ambiguous_json.contains("matched_value"));
        assert!(!classified_json.contains("source"));
        assert!(!ambiguous_json.contains("source"));
    }
}
