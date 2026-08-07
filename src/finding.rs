//! Public finding model produced by a scan.

use crate::{confidence::Confidence, location::Location, rule::RuleId, severity::Severity};

/// A single detection produced by a scanner.
///
/// A finding identifies the rule that matched, the exact source location of
/// the match, and the rule-assigned severity and confidence.
///
/// Findings do not retain or copy the matched secret. Consumers can inspect
/// the original source using [`Location::start`] and [`Location::end`] when
/// they explicitly need access to the matched span.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Finding {
    rule_id: RuleId,
    location: Location,
    severity: Severity,
    confidence: Confidence,
}

impl Finding {
    pub(crate) const fn new(
        rule_id: RuleId,
        location: Location,
        severity: Severity,
        confidence: Confidence,
    ) -> Self {
        Self {
            rule_id,
            location,
            severity,
            confidence,
        }
    }

    /// Returns the stable identifier of the rule that produced this finding.
    #[must_use]
    pub const fn rule_id(&self) -> &RuleId {
        &self.rule_id
    }

    /// Returns the exact source location of the detected span.
    #[must_use]
    pub const fn location(&self) -> &Location {
        &self.location
    }

    /// Returns the severity assigned by the matching rule.
    #[must_use]
    pub const fn severity(&self) -> Severity {
        self.severity
    }

    /// Returns the confidence assigned by the matching rule.
    #[must_use]
    pub const fn confidence(&self) -> Confidence {
        self.confidence
    }

    /// Returns `true` when this finding has critical severity.
    #[must_use]
    pub const fn is_critical(&self) -> bool {
        matches!(self.severity, Severity::Critical)
    }

    /// Returns `true` when this finding has high or critical severity.
    #[must_use]
    pub const fn is_high_priority(&self) -> bool {
        self.severity.is_high_priority()
    }

    /// Returns `true` when this finding has high confidence.
    #[must_use]
    pub const fn is_high_confidence(&self) -> bool {
        self.confidence.is_high()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_immutable_finding_metadata() {
        let finding = Finding::new(
            RuleId::from("example"),
            Location::from_span(2, 8),
            Severity::High,
            Confidence::High,
        );

        assert_eq!(finding.rule_id().as_str(), "example");
        assert_eq!(finding.location().start(), 2);
        assert_eq!(finding.location().end(), 8);
        assert_eq!(finding.severity(), Severity::High);
        assert_eq!(finding.confidence(), Confidence::High);
        assert!(!finding.is_critical());
        assert!(finding.is_high_priority());
        assert!(finding.is_high_confidence());
    }

    #[test]
    fn detects_critical_findings() {
        let finding = Finding::new(
            RuleId::from("critical"),
            Location::from_span(0, 1),
            Severity::Critical,
            Confidence::Medium,
        );

        assert!(finding.is_critical());
        assert!(finding.is_high_priority());
        assert!(!finding.is_high_confidence());
    }
}
