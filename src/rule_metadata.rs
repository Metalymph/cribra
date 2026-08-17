use crate::{Remediation, RuleKind, Severity};

/// Presentation-safe metadata describing a scan rule.
///
/// `RuleMetadata` exposes stable rule characteristics without exposing
/// matcher payloads, validator internals, regular expressions, or source data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RuleMetadata<'a> {
    id: &'a str,
    kind: RuleKind,
    severity: Severity,
    remediation: Option<Remediation>,
}

impl<'a> RuleMetadata<'a> {
    /// Creates metadata for a rule.
    #[must_use]
    pub const fn new(
        id: &'a str,
        kind: RuleKind,
        severity: Severity,
        remediation: Option<Remediation>,
    ) -> Self {
        Self {
            id,
            kind,
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
            Severity::High,
            Some(Remediation::RotateCredential),
        );

        assert_eq!(metadata.id(), "github.token");
        assert_eq!(metadata.kind(), RuleKind::Prefix);
        assert_eq!(metadata.severity(), Severity::High);
        assert_eq!(metadata.remediation(), Some(Remediation::RotateCredential));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trip_preserves_metadata() {
        let metadata = RuleMetadata::new(
            "stripe.secret",
            RuleKind::Prefix,
            Severity::Critical,
            Some(Remediation::RotateCredential),
        );

        let json = serde_json::to_string(&metadata).expect("serialize metadata");
        let decoded: RuleMetadata<'_> = serde_json::from_str(&json).expect("deserialize metadata");

        assert_eq!(decoded, metadata);
    }
}
