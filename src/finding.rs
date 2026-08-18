//! Public finding model produced by a scan.

use core::fmt;

use crate::{
    Explanation, Scanner, confidence::Confidence, location::Location, remediation::Remediation,
    rule::RuleId, severity::Severity,
};

/// A single detection produced by a scanner.
///
/// A finding identifies the rule that matched, the exact source location of
/// the match, and the rule-assigned severity and confidence.
///
/// Findings do not retain or copy the matched secret. Consumers can inspect
/// the original source using [`Location::start`] and [`Location::end`] when
/// they explicitly need access to the matched span.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Finding {
    rule_id: RuleId,
    location: Location,
    severity: Severity,
    confidence: Confidence,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    remediation: Option<Remediation>,
}

impl Finding {
    pub(crate) const fn new(
        rule_id: RuleId,
        location: Location,
        severity: Severity,
        confidence: Confidence,
        remediation: Option<Remediation>,
    ) -> Self {
        Self {
            rule_id,
            location,
            severity,
            confidence,
            remediation,
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

    /// Returns the recommended response to this finding, when one is known.
    #[must_use]
    pub const fn remediation(&self) -> Option<Remediation> {
        self.remediation
    }

    /// Resolves typed explanation facts against the scanner that produced this finding.
    ///
    /// A finding intentionally stores no duplicate detection-mode or explanation
    /// metadata. Resolution uses the scanner's immutable compiled rule metadata
    /// and fails closed when the finding cannot be mapped unambiguously.
    ///
    /// This operation performs no allocation and never accesses source text or
    /// the matched sensitive value.
    #[must_use]
    pub fn explanation(&self, scanner: &Scanner) -> Option<Explanation> {
        Explanation::for_finding(scanner, self)
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

impl fmt::Display for Finding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} · {} confidence · {} · {}:{}",
            self.severity,
            self.confidence,
            self.rule_id,
            self.location.line(),
            self.location.column(),
        )?;

        if let Some(remediation) = self.remediation {
            write!(formatter, " · {}", remediation.label())?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DetectionMode, Rule};

    #[test]
    fn exposes_immutable_finding_metadata() {
        let finding = Finding::new(
            RuleId::from("example"),
            Location::from_span(2, 8),
            Severity::High,
            Confidence::High,
            None,
        );

        assert_eq!(finding.rule_id().as_str(), "example");
        assert_eq!(finding.location().start(), 2);
        assert_eq!(finding.location().end(), 8);
        assert_eq!(finding.severity(), Severity::High);
        assert_eq!(finding.confidence(), Confidence::High);
        assert_eq!(finding.remediation(), None);
        assert!(!finding.is_critical());
        assert!(finding.is_high_priority());
        assert!(finding.is_high_confidence());
    }

    #[test]
    fn resolves_explanation_without_storing_duplicate_rule_metadata() {
        let scanner = Scanner::builder()
            .rule(Rule::literal("example", "secret", Severity::High))
            .build()
            .expect("scanner should compile");
        let results = scanner.scan([("memory", "secret")]);
        let finding = &results
            .single_report()
            .expect("one source was scanned")
            .findings()[0];

        assert_eq!(
            finding.explanation(&scanner),
            Some(Explanation::Classified(DetectionMode::MatcherOnly))
        );
    }

    #[test]
    fn explanation_fails_closed_against_unrelated_scanner() {
        let scanner = Scanner::builder().build().expect("scanner should compile");
        let finding = Finding::new(
            RuleId::from("example"),
            Location::from_span(2, 8),
            Severity::High,
            Confidence::High,
            None,
        );

        assert_eq!(finding.explanation(&scanner), None);
    }

    #[test]
    fn detects_critical_findings() {
        let finding = Finding::new(
            RuleId::from("critical"),
            Location::from_span(0, 1),
            Severity::Critical,
            Confidence::Medium,
            Some(Remediation::RemoveSensitiveValue),
        );

        assert!(finding.is_critical());
        assert_eq!(
            finding.remediation(),
            Some(Remediation::RemoveSensitiveValue),
        );
        assert!(finding.is_high_priority());
        assert!(!finding.is_high_confidence());
    }
}
