//! One-way projections of immutable Core metadata.

use crate::{Confidence, Finding, Location, RuleId, ScanReport, ScanSummary, Severity, SourceId};

impl From<&cribra::Location> for Location {
    fn from(value: &cribra::Location) -> Self {
        Self {
            start: value.start() as u64,
            end: value.end() as u64,
            line: value.line() as u64,
            column: value.column() as u64,
        }
    }
}

impl From<cribra::Severity> for Severity {
    fn from(value: cribra::Severity) -> Self {
        match value {
            cribra::Severity::Info => Self::Info,
            cribra::Severity::Low => Self::Low,
            cribra::Severity::Medium => Self::Medium,
            cribra::Severity::High => Self::High,
            cribra::Severity::Critical => Self::Critical,
        }
    }
}

impl From<cribra::Confidence> for Confidence {
    fn from(value: cribra::Confidence) -> Self {
        match value {
            cribra::Confidence::Low => Self::Low,
            cribra::Confidence::Medium => Self::Medium,
            cribra::Confidence::High => Self::High,
        }
    }
}

impl From<&cribra::RuleId> for RuleId {
    fn from(value: &cribra::RuleId) -> Self {
        Self {
            value: value.as_str().to_owned(),
        }
    }
}

impl From<&cribra::Finding> for Finding {
    fn from(value: &cribra::Finding) -> Self {
        Self {
            rule_id: value.rule_id().into(),
            location: value.location().into(),
            severity: value.severity().into(),
            confidence: value.confidence().into(),
        }
    }
}

impl ScanReport {
    pub(crate) fn from_parts(
        source_id: &SourceId,
        source_bytes: usize,
        report: &cribra::ScanReport,
    ) -> Self {
        Self {
            source_id: source_id.clone(),
            source_bytes: source_bytes as u64,
            findings: report.findings().iter().map(Finding::from).collect(),
            candidate_count: report.candidate_len() as u64,
            needs_review: report.needs_review(),
        }
    }
}

impl From<&cribra::ScanEntry<SourceId>> for ScanReport {
    fn from(entry: &cribra::ScanEntry<SourceId>) -> Self {
        Self::from_parts(entry.key(), entry.source_bytes(), entry.report())
    }
}

impl From<cribra::ScanSummary> for ScanSummary {
    fn from(value: cribra::ScanSummary) -> Self {
        Self {
            scanned_sources: value.scanned_sources() as u64,
            scanned_bytes: value.scanned_bytes() as u64,
            reports_with_findings: value.reports_with_findings() as u64,
            reports_without_findings: value.reports_without_findings() as u64,
            total_findings: value.total_findings() as u64,
            total_candidates: value.total_candidates() as u64,
            reports_with_candidates: value.reports_with_candidates() as u64,
            critical: value.critical() as u64,
            high: value.high() as u64,
            medium: value.medium() as u64,
            low: value.low() as u64,
            info: value.info() as u64,
        }
    }
}
