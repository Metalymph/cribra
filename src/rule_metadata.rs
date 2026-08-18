use crate::{Explanation, Remediation, RuleKind, Severity};

/// Describes how a rule decides whether a matched candidate should become a finding.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum DetectionMode {
    /// The matcher itself is authoritative and no built-in validator is applied.
    MatcherOnly,
    /// Validation depends only on the matched candidate's own structure.
    Deterministic,
    /// Validation also depends on surrounding source context.
    Contextual,
}

/// Presentation-safe metadata describing a scan rule.
///
/// `RuleMetadata` exposes stable rule characteristics without exposing
/// matcher payloads, validator internals, regular expressions, or source data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RuleMetadata<'a> {
    id: &'a str,
    kind: RuleKind,
    detection_mode: DetectionMode,
    severity: Severity,
    remediation: Option<Remediation>,
}

impl<'a> RuleMetadata<'a> {
    /// Creates metadata for a rule.
    #[must_use]
    pub const fn new(
        id: &'a str,
        kind: RuleKind,
        detection_mode: DetectionMode,
        severity: Severity,
        remediation: Option<Remediation>,
    ) -> Self {
        Self {
            id,
            kind,
            detection_mode,
            severity,
            remediation,
        }
    }

    /// Returns the stable rule identifier.
    #[must_use]
    pub const fn id(&self) -> &'a str {
        self.id
    }

    /// Returns the rule matching family.
    #[must_use]
    pub const fn kind(&self) -> RuleKind {
        self.kind
    }

    /// Returns how this rule validates matched candidates.
    #[must_use]
    pub const fn detection_mode(&self) -> DetectionMode {
        self.detection_mode
    }

    /// Returns typed explanation facts for findings produced by this rule.
    ///
    /// The explanation is projected directly from [`DetectionMode`], which is
    /// already the public authority describing how the rule validates matched
    /// candidates. No separate evidence classification is stored.
    #[must_use]
    pub const fn explanation(&self) -> Explanation {
        Explanation::classified(self.detection_mode)
    }

    /// Returns the default severity assigned by the rule.
    #[must_use]
    pub const fn severity(&self) -> Severity {
        self.severity
    }

    /// Returns presentation-safe remediation guidance, when available.
    #[must_use]
    pub const fn remediation(&self) -> Option<Remediation> {
        self.remediation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_presentation_safe_rule_metadata() {
        let metadata = RuleMetadata::new(
            "github.token",
            RuleKind::Prefix,
            DetectionMode::Deterministic,
            Severity::High,
            Some(Remediation::RotateCredential),
        );

        assert_eq!(metadata.id(), "github.token");
        assert_eq!(metadata.kind(), RuleKind::Prefix);
        assert_eq!(metadata.detection_mode(), DetectionMode::Deterministic);
        assert_eq!(
            metadata.explanation(),
            Explanation::Classified(DetectionMode::Deterministic)
        );
        assert_eq!(metadata.severity(), Severity::High);
        assert_eq!(metadata.remediation(), Some(Remediation::RotateCredential));
    }

    #[test]
    fn explanation_tracks_detection_mode_without_parallel_state() {
        for mode in [
            DetectionMode::MatcherOnly,
            DetectionMode::Deterministic,
            DetectionMode::Contextual,
        ] {
            let metadata =
                RuleMetadata::new("rule", RuleKind::Pattern, mode, Severity::Medium, None);

            assert_eq!(metadata.explanation(), Explanation::classified(mode));
            assert_eq!(metadata.explanation().detection_mode(), Some(mode));
            assert!(metadata.explanation().is_classified());
            assert!(!metadata.explanation().is_ambiguous());
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trip_preserves_metadata() {
        let metadata = RuleMetadata::new(
            "stripe.secret",
            RuleKind::Prefix,
            DetectionMode::Deterministic,
            Severity::Critical,
            Some(Remediation::RotateCredential),
        );

        let json = serde_json::to_string(&metadata).expect("serialize metadata");
        let decoded: RuleMetadata<'_> = serde_json::from_str(&json).expect("deserialize metadata");

        assert_eq!(decoded, metadata);
    }
}
