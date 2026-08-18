//! Immutable findings produced for one source in a batch scan.

use core::fmt;

use crate::{SensitiveCandidate, finding::Finding, severity::Severity};

/// Findings produced by scanning one UTF-8 source.
///
/// Findings are stored in deterministic source order. A report owns its
/// findings and cannot be mutated after construction.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScanReport {
    findings: Box<[Finding]>,
    candidates: Box<[SensitiveCandidate]>,
}

impl ScanReport {
    pub(crate) fn new_with_candidates(
        findings: Vec<Finding>,
        candidates: Vec<SensitiveCandidate>,
    ) -> Self {
        Self {
            findings: findings.into_boxed_slice(),
            candidates: candidates.into_boxed_slice(),
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

    /// Returns ambiguous sensitive candidates in deterministic source order.
    ///
    /// Candidates are review-only structural observations. They are not
    /// findings and do not carry severity, finding confidence or remediation.
    #[must_use]
    pub fn candidates(&self) -> &[SensitiveCandidate] {
        &self.candidates
    }

    /// Returns the number of ambiguous sensitive candidates in this report.
    #[must_use]
    pub fn candidate_len(&self) -> usize {
        self.candidates.len()
    }

    /// Returns `true` when this report contains at least one ambiguous
    /// sensitive candidate.
    #[must_use]
    pub fn has_candidates(&self) -> bool {
        !self.candidates.is_empty()
    }

    /// Returns `true` when either a finding or an ambiguous sensitive candidate
    /// requires review.
    #[must_use]
    pub fn needs_review(&self) -> bool {
        !self.findings.is_empty() || !self.candidates.is_empty()
    }

    /// Returns `true` when this report contains no findings.
    ///
    /// Ambiguous sensitive candidates are intentionally not findings and
    /// therefore do not affect this compatibility helper. Use
    /// [`ScanReport::needs_review`] when candidates must also be considered.
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

    /// Consumes the report and returns its ambiguous sensitive candidates.
    #[must_use]
    pub fn into_candidates(self) -> Box<[SensitiveCandidate]> {
        self.candidates
    }
}

impl fmt::Display for ScanReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.findings.is_empty() {
            formatter.write_str("no findings")?;
        } else {
            for (index, finding) in self.findings.iter().enumerate() {
                if index != 0 {
                    formatter.write_str("\n")?;
                }
                finding.fmt(formatter)?;
            }
        }

        if !self.candidates.is_empty() {
            write!(
                formatter,
                "\nambiguous candidates: {}",
                self.candidates.len()
            )?;
        }

        Ok(())
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
            None,
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
        let report = ScanReport::new_with_candidates(
            vec![
                finding("low", Severity::Low),
                finding("critical", Severity::Critical),
            ],
            Vec::new(),
        );

        assert!(report.has_critical());
    }

    #[test]
    fn filters_by_severity_without_cloning() {
        let report = ScanReport::new_with_candidates(
            vec![
                finding("low", Severity::Low),
                finding("high", Severity::High),
                finding("high-two", Severity::High),
            ],
            Vec::new(),
        );

        let ids = report
            .by_severity(Severity::High)
            .map(|finding| finding.rule_id().as_str())
            .collect::<Vec<_>>();

        assert_eq!(ids, ["high", "high-two"]);
    }
    fn sensitive_candidate() -> SensitiveCandidate {
        SensitiveCandidate::new(
            crate::SensitiveCandidateKind::RecoveryLikeCode,
            Location::from_span(2, 21),
            crate::CandidateEvidence::Structural,
        )
    }

    #[test]
    fn candidates_remain_separate_from_findings() {
        let report = ScanReport::new_with_candidates(Vec::new(), vec![sensitive_candidate()]);

        assert!(report.is_empty());
        assert_eq!(report.len(), 0);
        assert_eq!(report.candidate_len(), 1);
        assert!(report.has_candidates());
        assert!(report.needs_review());
        assert!(!report.has_critical());
    }

    #[test]
    fn candidate_only_report_does_not_iterate_as_findings() {
        let report = ScanReport::new_with_candidates(Vec::new(), vec![sensitive_candidate()]);

        assert_eq!(report.iter().count(), 0);
        assert_eq!((&report).into_iter().count(), 0);
        assert_eq!(report.candidates().len(), 1);
    }

    #[test]
    fn display_distinguishes_candidates_from_findings() {
        let report = ScanReport::new_with_candidates(Vec::new(), vec![sensitive_candidate()]);

        assert_eq!(report.to_string(), "no findings\nambiguous candidates: 1");
    }
}
