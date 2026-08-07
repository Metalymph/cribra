//! Aggregate statistics for a batch scan.

use std::fmt;

use crate::Severity;

/// Aggregate statistics derived from [`ScanResults`](crate::ScanResults).
///
/// A summary contains only counters and byte totals. It never stores source
/// text, findings, source keys or matched secret values.
#[derive(Debug, Copy, Clone, Default, Eq, PartialEq, Hash)]
pub struct ScanSummary {
    scanned_sources: usize,
    scanned_bytes: usize,
    reports_with_findings: usize,
    reports_without_findings: usize,
    total_findings: usize,
    critical: usize,
    high: usize,
    medium: usize,
    low: usize,
    info: usize,
}

/// Internal accumulator used while deriving a [`ScanSummary`].
///
/// Keeping accumulation separate from the immutable public value avoids a
/// wide positional constructor and makes future counters easier to add without
/// changing call sites.
#[derive(Debug, Copy, Clone, Default, Eq, PartialEq)]
pub(crate) struct ScanSummaryStats {
    pub(crate) scanned_sources: usize,
    pub(crate) scanned_bytes: usize,
    pub(crate) reports_with_findings: usize,
    pub(crate) reports_without_findings: usize,
    pub(crate) total_findings: usize,
    pub(crate) critical: usize,
    pub(crate) high: usize,
    pub(crate) medium: usize,
    pub(crate) low: usize,
    pub(crate) info: usize,
}

impl ScanSummary {
    /// Materializes an immutable public summary from internal accumulated
    /// statistics.
    pub(crate) const fn from_stats(stats: ScanSummaryStats) -> Self {
        Self {
            scanned_sources: stats.scanned_sources,
            scanned_bytes: stats.scanned_bytes,
            reports_with_findings: stats.reports_with_findings,
            reports_without_findings: stats.reports_without_findings,
            total_findings: stats.total_findings,
            critical: stats.critical,
            high: stats.high,
            medium: stats.medium,
            low: stats.low,
            info: stats.info,
        }
    }

    /// Returns the number of scanned sources.
    #[must_use]
    pub const fn scanned_sources(self) -> usize {
        self.scanned_sources
    }

    /// Returns the total number of UTF-8 source bytes scanned.
    #[must_use]
    pub const fn scanned_bytes(self) -> usize {
        self.scanned_bytes
    }

    /// Returns the number of source reports containing at least one finding.
    #[must_use]
    pub const fn reports_with_findings(self) -> usize {
        self.reports_with_findings
    }

    /// Returns the number of source reports containing no findings.
    #[must_use]
    pub const fn reports_without_findings(self) -> usize {
        self.reports_without_findings
    }

    /// Returns the total number of findings across all sources.
    #[must_use]
    pub const fn total_findings(self) -> usize {
        self.total_findings
    }

    /// Returns the number of findings with the requested severity.
    #[must_use]
    pub const fn by_severity(self, severity: Severity) -> usize {
        match severity {
            Severity::Critical => self.critical,
            Severity::High => self.high,
            Severity::Medium => self.medium,
            Severity::Low => self.low,
            Severity::Info => self.info,
        }
    }

    /// Returns the number of critical findings.
    #[must_use]
    pub const fn critical(self) -> usize {
        self.critical
    }

    /// Returns the number of high-severity findings.
    #[must_use]
    pub const fn high(self) -> usize {
        self.high
    }

    /// Returns the number of medium-severity findings.
    #[must_use]
    pub const fn medium(self) -> usize {
        self.medium
    }

    /// Returns the number of low-severity findings.
    #[must_use]
    pub const fn low(self) -> usize {
        self.low
    }

    /// Returns the number of informational findings.
    #[must_use]
    pub const fn info(self) -> usize {
        self.info
    }

    /// Returns `true` when the batch contains no findings.
    #[must_use]
    pub const fn is_clean(self) -> bool {
        self.total_findings == 0
    }

    /// Returns `true` when the batch contains at least one critical finding.
    #[must_use]
    pub const fn has_critical(self) -> bool {
        self.critical != 0
    }
}

impl fmt::Display for ScanSummary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            concat!(
                "sources: {} (failed: {}, clean: {})\n",
                "bytes: {}\n",
                "findings: {}\n",
                "critical: {}\n",
                "high: {}\n",
                "medium: {}\n",
                "low: {}\n",
                "info: {}"
            ),
            self.scanned_sources,
            self.reports_with_findings,
            self.reports_without_findings,
            self.scanned_bytes,
            self.total_findings,
            self.critical,
            self.high,
            self.medium,
            self.low,
            self.info,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_counts_and_status_helpers() {
        let summary = ScanSummary::from_stats(ScanSummaryStats {
            scanned_sources: 3,
            scanned_bytes: 128,
            reports_with_findings: 2,
            reports_without_findings: 1,
            total_findings: 5,
            critical: 1,
            high: 2,
            medium: 1,
            low: 1,
            info: 0,
        });

        assert_eq!(summary.scanned_sources(), 3);
        assert_eq!(summary.scanned_bytes(), 128);
        assert_eq!(summary.reports_with_findings(), 2);
        assert_eq!(summary.reports_without_findings(), 1);
        assert_eq!(summary.total_findings(), 5);
        assert_eq!(summary.by_severity(Severity::Critical), 1);
        assert_eq!(summary.by_severity(Severity::High), 2);
        assert_eq!(summary.by_severity(Severity::Medium), 1);
        assert_eq!(summary.by_severity(Severity::Low), 1);
        assert_eq!(summary.by_severity(Severity::Info), 0);
        assert!(summary.has_critical());
        assert!(!summary.is_clean());
    }

    #[test]
    fn formats_stable_plain_text_summary() {
        let summary = ScanSummary::from_stats(ScanSummaryStats {
            scanned_sources: 2,
            scanned_bytes: 64,
            reports_with_findings: 1,
            reports_without_findings: 1,
            total_findings: 1,
            critical: 1,
            high: 0,
            medium: 0,
            low: 0,
            info: 0,
        });

        assert_eq!(
            summary.to_string(),
            concat!(
                "sources: 2 (failed: 1, clean: 1)\n",
                "bytes: 64\n",
                "findings: 1\n",
                "critical: 1\n",
                "high: 0\n",
                "medium: 0\n",
                "low: 0\n",
                "info: 0"
            ),
        );
    }
}
