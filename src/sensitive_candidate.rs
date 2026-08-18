//! Public model for structurally plausible sensitive values.
//!
//! Sensitive candidates are intentionally separate from [`Finding`](crate::Finding).
//! A candidate means that a source span has a shape worth reviewing, but the
//! scanner does not have enough evidence to classify it as a detected secret or
//! credential.
//!
//! Candidates never retain or copy the source value.

use crate::Location;

/// Describes the kind of sensitive value a candidate resembles.
///
/// This enum is non-exhaustive so future versions can add carefully validated
/// candidate families without forcing consumers to treat today's set as closed.
#[non_exhaustive]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum SensitiveCandidateKind {
    /// A value whose structure resembles a recovery or backup code.
    RecoveryLikeCode,
}

/// Describes the evidence that caused a value to be surfaced as a candidate.
///
/// Candidate evidence is deliberately different from
/// [`Confidence`](crate::Confidence). Confidence applies to an actual finding;
/// candidate evidence describes why a value is worth manual review before the
/// scanner has enough evidence to call it a detection.
#[non_exhaustive]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum CandidateEvidence {
    /// The value has a recognizable sensitive-looking structure, but lacks
    /// sufficient semantic context for a finding.
    Structural,
}

/// A structurally plausible sensitive value that requires manual review.
///
/// `SensitiveCandidate` is not a [`Finding`](crate::Finding):
///
/// - it has no severity;
/// - it has no finding confidence;
/// - it has no remediation;
/// - it does not imply that a credential was detected.
///
/// The candidate stores only presentation-safe metadata and an exact source
/// location. It never stores the matched source text.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SensitiveCandidate {
    kind: SensitiveCandidateKind,
    location: Location,
    evidence: CandidateEvidence,
}

impl SensitiveCandidate {
    pub(crate) const fn new(
        kind: SensitiveCandidateKind,
        location: Location,
        evidence: CandidateEvidence,
    ) -> Self {
        Self {
            kind,
            location,
            evidence,
        }
    }

    /// Returns the family this candidate structurally resembles.
    #[must_use]
    pub const fn kind(&self) -> SensitiveCandidateKind {
        self.kind
    }

    /// Returns the exact source location of the candidate span.
    #[must_use]
    pub const fn location(&self) -> &Location {
        &self.location
    }

    /// Returns the evidence that caused this value to be surfaced for review.
    #[must_use]
    pub const fn evidence(&self) -> CandidateEvidence {
        self.evidence
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_candidate_metadata_without_finding_semantics() {
        let candidate = SensitiveCandidate::new(
            SensitiveCandidateKind::RecoveryLikeCode,
            Location::from_span(4, 23),
            CandidateEvidence::Structural,
        );

        assert_eq!(candidate.kind(), SensitiveCandidateKind::RecoveryLikeCode);
        assert_eq!(candidate.location().start(), 4);
        assert_eq!(candidate.location().end(), 23);
        assert_eq!(candidate.evidence(), CandidateEvidence::Structural);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trip_preserves_candidate_metadata() {
        let candidate = SensitiveCandidate::new(
            SensitiveCandidateKind::RecoveryLikeCode,
            Location::from_span(2, 21),
            CandidateEvidence::Structural,
        );

        let json = serde_json::to_string(&candidate).expect("serialize candidate");
        let decoded: SensitiveCandidate =
            serde_json::from_str(&json).expect("deserialize candidate");

        assert_eq!(decoded, candidate);
        assert!(!json.contains("matched_value"));
        assert!(!json.contains("secret"));
    }
}
