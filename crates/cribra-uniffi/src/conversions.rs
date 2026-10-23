//! One-way projections of immutable Core metadata.

use crate::{
    CandidateEvidence, Confidence, DetectionMode, Explanation, Finding, Location, Remediation,
    RuleId, ScanReport, ScanSummary, SensitiveCandidate, SensitiveCandidateKind, Severity,
    SourceId,
};

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
            remediation: value.remediation().map(Into::into),
            explanation: None,
        }
    }
}

impl Finding {
    /// Projects an original Core finding, resolving explanation only through Core.
    /// Incompatible scanner metadata returns None, exactly as Core does.
    pub fn from_core(value: &cribra::Finding, scanner: &cribra::Scanner) -> Self {
        Self {
            explanation: value.explanation(scanner).and_then(Explanation::from_core),
            ..Self::from(value)
        }
    }
}

impl ScanReport {
    pub(crate) fn from_parts(
        source_id: &SourceId,
        source_bytes: usize,
        report: &cribra::ScanReport,
        scanner: Option<&cribra::Scanner>,
    ) -> Self {
        Self {
            source_id: source_id.clone(),
            source_bytes: source_bytes as u64,
            findings: report
                .findings()
                .iter()
                .map(|finding| {
                    scanner.map_or_else(
                        || Finding::from(finding),
                        |scanner| Finding::from_core(finding, scanner),
                    )
                })
                .collect(),
            candidates: report
                .candidates()
                .iter()
                .map(SensitiveCandidate::from)
                .collect(),
            candidate_count: report.candidate_len() as u64,
            needs_review: report.needs_review(),
        }
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

impl From<cribra::Remediation> for Remediation {
    fn from(value: cribra::Remediation) -> Self {
        match value {
            cribra::Remediation::RevokeAndRotateCredential => Self::RevokeAndRotateCredential,
            cribra::Remediation::RotateCredential => Self::RotateCredential,
            cribra::Remediation::RotatePassword => Self::RotatePassword,
            cribra::Remediation::ReplacePrivateKey => Self::ReplacePrivateKey,
            cribra::Remediation::RemoveSensitiveValue => Self::RemoveSensitiveValue,
            cribra::Remediation::ReviewSensitiveHash => Self::ReviewSensitiveHash,
            cribra::Remediation::ReviewPasswordVerifier => Self::ReviewPasswordVerifier,
            _ => panic!("unsupported Core remediation"),
        }
    }
}

impl From<cribra::DetectionMode> for DetectionMode {
    fn from(value: cribra::DetectionMode) -> Self {
        match value {
            cribra::DetectionMode::MatcherOnly => Self::MatcherOnly,
            cribra::DetectionMode::Deterministic => Self::Deterministic,
            cribra::DetectionMode::Contextual => Self::Contextual,
        }
    }
}

impl From<cribra::CandidateEvidence> for CandidateEvidence {
    fn from(value: cribra::CandidateEvidence) -> Self {
        match value {
            cribra::CandidateEvidence::Structural => Self::Structural,
            _ => panic!("unsupported Core candidate evidence"),
        }
    }
}

impl Explanation {
    fn from_core(value: cribra::Explanation) -> Option<Self> {
        match value {
            cribra::Explanation::Classified(mode) => Some(Self::Classified { mode: mode.into() }),
            cribra::Explanation::Ambiguous(cribra::CandidateEvidence::Structural) => {
                Some(Self::Ambiguous {
                    evidence: CandidateEvidence::Structural,
                })
            }
            _ => None,
        }
    }
}

impl From<&cribra::SensitiveCandidate> for SensitiveCandidate {
    fn from(value: &cribra::SensitiveCandidate) -> Self {
        Self {
            kind: match value.kind() {
                cribra::SensitiveCandidateKind::RecoveryLikeCode => {
                    SensitiveCandidateKind::RecoveryLikeCode
                }
                _ => panic!("unsupported Core candidate kind"),
            },
            location: value.location().into(),
            evidence: value.evidence().into(),
            explanation: Explanation::from_core(value.explanation()),
        }
    }
}

// Preserve the existing source-free projection when no scanner was supplied.
// Its finding explanations stay unresolved rather than inferring authority.
impl From<&cribra::ScanEntry<SourceId>> for ScanReport {
    fn from(entry: &cribra::ScanEntry<SourceId>) -> Self {
        Self::from_parts(entry.key(), entry.source_bytes(), entry.report(), None)
    }
}
