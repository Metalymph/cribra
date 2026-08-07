//! Ordered results returned by [`Scanner::scan`](crate::Scanner::scan).

use crate::{
    Finding, ScanEntry, ScanReport, ScanSummary, Severity, scan_summary::ScanSummaryStats,
};

/// Ordered results for a batch of identified UTF-8 sources.
///
/// Entries preserve input order. The collection owns caller-supplied keys and
/// reports, but never stores source text.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct ScanResults<K> {
    entries: Vec<ScanEntry<K>>,
}

impl<K> ScanResults<K> {
    pub(crate) const fn new(entries: Vec<ScanEntry<K>>) -> Self {
        Self { entries }
    }

    /// Returns the number of scanned sources.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` when no sources were scanned.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns all entries in input order.
    #[must_use]
    pub fn as_slice(&self) -> &[ScanEntry<K>] {
        &self.entries
    }

    /// Iterates over entries in input order.
    pub fn iter(&self) -> std::slice::Iter<'_, ScanEntry<K>> {
        self.entries.iter()
    }

    /// Iterates over every finding while retaining its source key.
    ///
    /// No finding or key is cloned.
    pub fn findings(&self) -> impl Iterator<Item = (&K, &Finding)> {
        self.entries.iter().flat_map(|entry| {
            entry
                .report()
                .iter()
                .map(move |finding| (entry.key(), finding))
        })
    }

    /// Returns the total number of UTF-8 source bytes scanned.
    ///
    /// This performs no allocation.
    #[must_use]
    pub fn total_bytes(&self) -> usize {
        self.entries.iter().map(ScanEntry::source_bytes).sum()
    }

    /// Returns the total number of findings across all source reports.
    ///
    /// This performs no allocation.
    #[must_use]
    pub fn total_findings(&self) -> usize {
        self.entries.iter().map(|entry| entry.report().len()).sum()
    }

    /// Returns `true` when any source report contains a critical finding.
    ///
    /// Iteration stops at the first critical report.
    #[must_use]
    pub fn has_critical(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.report().has_critical())
    }

    /// Iterates over source entries containing at least one finding.
    ///
    /// No entry, key, report or finding is cloned.
    pub fn failed(&self) -> impl DoubleEndedIterator<Item = &ScanEntry<K>> {
        self.entries
            .iter()
            .filter(|entry| !entry.report().is_empty())
    }

    /// Iterates over source entries containing no findings.
    ///
    /// No entry, key or report is cloned.
    pub fn clean(&self) -> impl DoubleEndedIterator<Item = &ScanEntry<K>> {
        self.entries
            .iter()
            .filter(|entry| entry.report().is_empty())
    }

    /// Computes aggregate statistics for the complete batch.
    ///
    /// The summary contains counters only and never retains source text,
    /// findings or matched values.
    #[must_use]
    pub fn summary(&self) -> ScanSummary {
        let mut stats = ScanSummaryStats {
            scanned_sources: self.entries.len(),
            ..ScanSummaryStats::default()
        };

        for entry in &self.entries {
            stats.scanned_bytes += entry.source_bytes();

            let report = entry.report();
            if report.is_empty() {
                stats.reports_without_findings += 1;
            } else {
                stats.reports_with_findings += 1;
            }

            for finding in report {
                stats.total_findings += 1;

                match finding.severity() {
                    Severity::Critical => stats.critical += 1,
                    Severity::High => stats.high += 1,
                    Severity::Medium => stats.medium += 1,
                    Severity::Low => stats.low += 1,
                    Severity::Info => stats.info += 1,
                }
            }
        }

        ScanSummary::from_stats(stats)
    }

    /// Consumes the collection and returns its entries.
    #[must_use]
    pub fn into_inner(self) -> Vec<ScanEntry<K>> {
        self.entries
    }

    /// Returns the only report when exactly one source was scanned.
    ///
    /// This is a convenience for callers that intentionally submit a
    /// single-element batch while keeping the public scanning model uniform.
    #[must_use]
    pub fn single_report(&self) -> Option<&ScanReport> {
        match self.entries.as_slice() {
            [entry] => Some(entry.report()),
            _ => None,
        }
    }
}

impl<'a, K> IntoIterator for &'a ScanResults<K> {
    type Item = &'a ScanEntry<K>;
    type IntoIter = std::slice::Iter<'a, ScanEntry<K>>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

impl<K> IntoIterator for ScanResults<K> {
    type Item = ScanEntry<K>;
    type IntoIter = std::vec::IntoIter<ScanEntry<K>>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_results_have_no_entries_or_findings() {
        let results = ScanResults::<&str>::default();

        assert!(results.is_empty());
        assert_eq!(results.len(), 0);
        assert_eq!(results.findings().count(), 0);
        assert!(results.single_report().is_none());
    }

    fn report_with(severities: &[Severity]) -> ScanReport {
        let findings = severities
            .iter()
            .enumerate()
            .map(|(index, severity)| {
                crate::Finding::new(
                    crate::RuleId::from(format!("rule-{index}")),
                    crate::Location::from_span(index, index + 1),
                    *severity,
                    crate::Confidence::High,
                )
            })
            .collect();

        ScanReport::new(findings)
    }

    #[test]
    fn exposes_batch_helpers_without_cloning() {
        let results = ScanResults::new(vec![
            ScanEntry::new("clean", 10, ScanReport::default()),
            ScanEntry::new(
                "failed",
                20,
                report_with(&[Severity::High, Severity::Critical]),
            ),
        ]);

        assert_eq!(results.total_bytes(), 30);
        assert_eq!(results.total_findings(), 2);
        assert!(results.has_critical());
        assert_eq!(results.clean().count(), 1);
        assert_eq!(results.failed().count(), 1);
        assert_eq!(results.clean().next().unwrap().key(), &"clean");
        assert_eq!(results.failed().next().unwrap().key(), &"failed");
    }

    #[test]
    fn summary_aggregates_sources_bytes_and_severities() {
        let results = ScanResults::new(vec![
            ScanEntry::new("clean", 5, ScanReport::default()),
            ScanEntry::new(
                "failed-a",
                7,
                report_with(&[Severity::Critical, Severity::High]),
            ),
            ScanEntry::new(
                "failed-b",
                11,
                report_with(&[Severity::Medium, Severity::Low, Severity::Info]),
            ),
        ]);

        let summary = results.summary();

        assert_eq!(summary.scanned_sources(), 3);
        assert_eq!(summary.scanned_bytes(), 23);
        assert_eq!(summary.reports_with_findings(), 2);
        assert_eq!(summary.reports_without_findings(), 1);
        assert_eq!(summary.total_findings(), 5);
        assert_eq!(summary.critical(), 1);
        assert_eq!(summary.high(), 1);
        assert_eq!(summary.medium(), 1);
        assert_eq!(summary.low(), 1);
        assert_eq!(summary.info(), 1);
        assert!(summary.has_critical());
        assert!(!summary.is_clean());
    }
}
