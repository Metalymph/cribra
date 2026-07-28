//! Immutable report returned by [`Scanner::scan`](crate::Scanner::scan).

use crate::{finding::Finding, severity::Severity};

/// Findings produced by scanning one UTF-8 source.
///
/// Findings are stored in deterministic source order. A report owns its
/// findings and cannot be mutated after construction.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct ScanReport {
    findings: Box<[Finding]>,
}

impl ScanReport {
    pub(crate) fn new(findings: Vec<Finding>) -> Self {
        Self {
            findings: findings.into_boxed_slice(),
        }
    }

    /// Returns all findings in deterministic order.
    #[must_use]
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    /// Returns the number of findings in this report.
    #[must_use]
    pub fn len(&self) -> usize {
        self.findings.len()
    }

    /// Returns `true` when this report contains no findings.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.findings.is_empty()
    }

    /// Returns `true` when at least one finding is critical.
    ///
    /// Iteration stops as soon as the first critical finding is encountered.
    #[must_use]
    pub fn has_critical(&self) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.severity() == Severity::Critical)
    }

    /// Iterates over findings with exactly the requested severity.
    ///
    /// This method performs no allocation and does not clone findings.
    ///
    /// Returns an iterator that yields references to [`Finding`]s with the given severity.
    pub fn by_severity(
        &self,
        severity: Severity,
    ) -> impl DoubleEndedIterator<Item = &Finding> + '_ {
        self.findings
            .iter()
            .filter(move |finding| finding.severity() == severity)
    }

    /// Iterates over all findings in deterministic order.
    pub fn iter(&self) -> std::slice::Iter<'_, Finding> {
        self.findings.iter()
    }

    /// Consumes the report and returns its owned findings.
    #[must_use]
    pub fn into_findings(self) -> Box<[Finding]> {
        self.findings
    }
}

impl<'a> IntoIterator for &'a ScanReport {
    type Item = &'a Finding;
    type IntoIter = std::slice::Iter<'a, Finding>;

    fn into_iter(self) -> Self::IntoIter {
        self.findings.iter()
    }
}

impl IntoIterator for ScanReport {
    type Item = Finding;
    type IntoIter = std::vec::IntoIter<Finding>;

    fn into_iter(self) -> Self::IntoIter {
        Vec::from(self.findings).into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Confidence, Location, RuleId};

    fn finding(id: &str, severity: Severity) -> Finding {
        Finding::new(
            RuleId::from(id),
            Location::from_span(0, 1),
            severity,
            Confidence::High,
        )
    }

    #[test]
    fn empty_report_has_no_findings() {
        let report = ScanReport::default();

        assert!(report.is_empty());
        assert_eq!(report.len(), 0);
        assert!(!report.has_critical());
    }

    #[test]
    fn detects_critical_findings() {
        let report = ScanReport::new(vec![
            finding("low", Severity::Low),
            finding("critical", Severity::Critical),
        ]);

        assert!(report.has_critical());
    }

    #[test]
    fn filters_by_severity_without_cloning() {
        let report = ScanReport::new(vec![
            finding("low", Severity::Low),
            finding("high", Severity::High),
            finding("high-two", Severity::High),
        ]);

        let ids = report
            .by_severity(Severity::High)
            .map(|finding| finding.rule_id().as_str())
            .collect::<Vec<_>>();

        assert_eq!(ids, ["high", "high-two"]);
    }
}
