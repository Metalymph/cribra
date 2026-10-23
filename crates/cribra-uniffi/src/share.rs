//! In-memory Share Bundles; scanning, validation and transformations belong to Core.

use crate::{
    Finding, Location, PseudonymizationOptions, ScanSource, ScanSummary, Scanner, SourceId,
    SynthesisOptions, TransformError,
};
use serde_json::json;
use std::{sync::Arc, time::UNIX_EPOCH};

/// Explicit transformation selection. Keyed modes reuse input-only Flow options.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum ShareMode {
    /// Core default redaction, including overlap merging.
    Redact,
    /// Core default semantic template.
    Template,
    /// Keyed Core pseudonymization.
    Pseudonymize {
        /// Explicit caller options.
        options: PseudonymizationOptions,
    },
    /// Keyed Core synthesis.
    Synthesize {
        /// Explicit caller options.
        options: SynthesisOptions,
    },
}

/// Key-free transformation kind stored in the manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ShareModeKind {
    /// Core Redact.
    Redact,
    /// Core Template.
    Template,
    /// Core Pseudonymize.
    Pseudonymize,
    /// Core Synthesize.
    Synthesize,
}

/// Owned builder configuration with no implicit transformation selection.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ShareBundleOptions {
    /// None preserves Core's MissingShareMode error.
    pub mode: Option<ShareMode>,
}

/// Candidate family, separate from classified findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ShareCandidateKind {
    /// Structurally plausible recovery/backup code.
    RecoveryLikeCode,
}
/// Evidence for a review-only candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ShareCandidateEvidence {
    /// Structural evidence, without finding confidence.
    Structural,
}
/// Complete source-free candidate metadata needed to preserve Core counters.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ShareCandidate {
    /// Candidate family.
    pub kind: ShareCandidateKind,
    /// Original UTF-8 coordinates.
    pub location: Location,
    /// Candidate evidence.
    pub evidence: ShareCandidateEvidence,
}
/// Owned precomputed report for build; no source text or redundant summary.
/// Unlike the earlier scanner projection, candidates are retained in full.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ShareReport {
    /// Caller identity; duplicates and empty IDs are preserved by position.
    pub source_id: SourceId,
    /// Recorded original UTF-8 byte length; checked only by Core.
    pub source_bytes: u64,
    /// Findings transferred unchanged, without rescanning.
    pub findings: Vec<Finding>,
    /// Review-only metadata; never transformed as findings.
    pub candidates: Vec<ShareCandidate>,
}
/// Exact Core generation time, without claiming reproducibility between calls.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ShareTimestamp {
    /// Whole seconds since the Unix epoch.
    pub seconds: u64,
    /// Nanoseconds within the second, less than one billion.
    pub nanoseconds: u32,
}
/// Aggregate metadata only; never contains keys, findings or source text.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ShareManifest {
    /// Transformation kind, without configuration/key material.
    pub mode: ShareModeKind,
    /// Core-derived original scan counters.
    pub summary: ScanSummary,
    /// Actual Core timestamp; not deterministic across calls.
    pub generated_at: ShareTimestamp,
}
/// One owned transformed artifact, in original batch order.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TransformedSource {
    /// Caller identity copied from the corresponding report.
    pub source_id: SourceId,
    /// Core-transformed text; text outside findings remains unchanged.
    pub content: String,
}
/// In-memory bundle only: no archive, serialized output or filesystem side effects.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ShareBundle {
    /// One artifact per source, preserving duplicate IDs and input order.
    pub sources: Vec<TransformedSource>,
    /// Core-generated manifest.
    pub manifest: ShareManifest,
}
/// Stable symbolic bundle errors; discriminants and diagnostic text are not stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ShareErrorCode {
    /// No explicit mode selected.
    MissingShareMode,
    /// Number of source strings differs from precomputed entries.
    SourceCountMismatch,
    /// Original byte length differs at one batch index.
    SourceLengthMismatch,
    /// A DTO or future Core value cannot be represented by this adapter.
    InputConversion,
}
/// No partial bundle is returned on any failure.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Error)]
pub enum ShareError {
    /// Reuses existing structured Flow errors (including invalid key lengths).
    Transform {
        /// Existing Flow failure.
        error: TransformError,
    },
    /// Bundle construction failure, with numeric context only.
    Rejected {
        /// Stable symbolic reason.
        code: ShareErrorCode,
        /// Expected source count or byte length, when applicable.
        expected: Option<u64>,
        /// Actual source count or byte length, when applicable.
        actual: Option<u64>,
        /// Zero-based batch index, for length mismatch.
        index: Option<u64>,
        /// Core diagnostic, or fixed representation failure; no input text/keys.
        detail: String,
    },
}
impl std::fmt::Display for ShareError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transform { error } => error.fmt(f),
            Self::Rejected { detail, .. } => f.write_str(detail),
        }
    }
}
impl std::error::Error for ShareError {}
impl From<TransformError> for ShareError {
    fn from(error: TransformError) -> Self {
        Self::Transform { error }
    }
}
impl From<cribra::transform::TransformError> for ShareError {
    fn from(error: cribra::transform::TransformError) -> Self {
        use cribra::transform::TransformError as E;
        let (code, expected, actual, index) = match &error {
            E::MissingShareMode => (ShareErrorCode::MissingShareMode, None, None, None),
            E::SourceCountMismatch { expected, actual } => (
                ShareErrorCode::SourceCountMismatch,
                Some(*expected as u64),
                Some(*actual as u64),
                None,
            ),
            E::SourceLengthMismatch {
                index,
                expected_bytes,
                actual_bytes,
            } => (
                ShareErrorCode::SourceLengthMismatch,
                Some(*expected_bytes as u64),
                Some(*actual_bytes as u64),
                Some(*index as u64),
            ),
            _ => {
                return Self::Transform {
                    error: error.into(),
                };
            }
        };
        Self::Rejected {
            code,
            expected,
            actual,
            index,
            detail: error.to_string(),
        }
    }
}
fn conversion() -> ShareError {
    ShareError::Rejected {
        code: ShareErrorCode::InputConversion,
        expected: None,
        actual: None,
        index: None,
        detail: "share metadata cannot be represented on this target".into(),
    }
}
impl ShareBundleOptions {
    fn into_core(self) -> Result<cribra::transform::ShareBundleBuilder, ShareError> {
        use cribra::transform::{ShareBundleBuilder, ShareMode as M};
        let builder = ShareBundleBuilder::new();
        Ok(match self.mode {
            None => builder,
            Some(mode) => builder.mode(match mode {
                ShareMode::Redact => M::Redact,
                ShareMode::Template => M::Template,
                ShareMode::Pseudonymize { options } => M::Pseudonymize(options.into_core()?),
                ShareMode::Synthesize { options } => M::Synthesize(options.into_core()?),
            }),
        })
    }
}
fn project(bundle: cribra::transform::ShareBundle<String>) -> Result<ShareBundle, ShareError> {
    use cribra::transform::ShareModeKind as K;
    let m = bundle.manifest();
    let mode = match m.mode() {
        K::Redact => ShareModeKind::Redact,
        K::Template => ShareModeKind::Template,
        K::Pseudonymize => ShareModeKind::Pseudonymize,
        K::Synthesize => ShareModeKind::Synthesize,
        _ => return Err(conversion()),
    };
    let time = m
        .generated_at()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| conversion())?;
    let manifest = ShareManifest {
        mode,
        summary: m.summary().into(),
        generated_at: ShareTimestamp {
            seconds: time.as_secs(),
            nanoseconds: time.subsec_nanos(),
        },
    };
    Ok(ShareBundle {
        sources: bundle
            .into_sources()
            .into_iter()
            .map(|s| {
                let (id, content) = s.into_parts();
                TransformedSource {
                    source_id: SourceId { value: id },
                    content,
                }
            })
            .collect(),
        manifest,
    })
}

#[uniffi::export]
impl Scanner {
    /// Scans once using Core and returns complete source-free inputs for build.
    /// No transformation or source storage is performed.
    pub fn scan_for_share(&self, sources: Vec<ScanSource>) -> Result<Vec<ShareReport>, ShareError> {
        let results = self.inner.scan(
            sources
                .iter()
                .map(|s| (s.id.value.clone(), s.text.as_str())),
        );
        results
            .iter()
            .map(|entry| {
                let candidates = entry
                    .report()
                    .candidates()
                    .iter()
                    .map(|c| {
                        let kind = match c.kind() {
                            cribra::SensitiveCandidateKind::RecoveryLikeCode => {
                                ShareCandidateKind::RecoveryLikeCode
                            }
                            _ => return Err(conversion()),
                        };
                        let evidence = match c.evidence() {
                            cribra::CandidateEvidence::Structural => {
                                ShareCandidateEvidence::Structural
                            }
                            _ => return Err(conversion()),
                        };
                        Ok(ShareCandidate {
                            kind,
                            evidence,
                            location: c.location().into(),
                        })
                    })
                    .collect::<Result<_, ShareError>>()?;
                Ok(ShareReport {
                    source_id: SourceId {
                        value: entry.key().clone(),
                    },
                    source_bytes: entry.source_bytes() as u64,
                    findings: entry
                        .report()
                        .findings()
                        .iter()
                        .map(|finding| Finding::from_core(finding, &self.inner))
                        .collect(),
                    candidates,
                })
            })
            .collect()
    }
}

/// Atomic Core scan-and-build, preserving source identity by construction.
#[uniffi::export]
pub fn scan_and_build(
    scanner: Arc<Scanner>,
    sources: Vec<ScanSource>,
    options: ShareBundleOptions,
) -> Result<ShareBundle, ShareError> {
    let builder = options.into_core()?;
    project(
        builder.scan_and_build(
            &scanner.inner,
            sources
                .iter()
                .map(|s| (s.id.value.clone(), s.text.as_str())),
        )?,
    )
}

/// Builds from explicit reports and positional source text without rescanning.
/// Core validates count and byte lengths, not source identity; equal-length changed
/// content is accepted. Core computes counters and validates transformation spans.
#[uniffi::export]
pub fn build(
    reports: Vec<ShareReport>,
    sources: Vec<String>,
    options: ShareBundleOptions,
) -> Result<ShareBundle, ShareError> {
    let builder = options.into_core()?;
    // Core constructors are private: transfer metadata through its public serde model.
    // No source text or cryptographic keys are serialized, and no findings/candidates are invented.
    let entries = reports.into_iter().map(|r| {
        let report = crate::flow::core_report(r.findings)?;
        let mut report = serde_json::to_value(report).map_err(|_| conversion())?;
        report["candidates"] = serde_json::Value::Array(r.candidates.into_iter().map(|c| json!({
            "kind": match c.kind { ShareCandidateKind::RecoveryLikeCode => "recovery_like_code" },
            "evidence": match c.evidence { ShareCandidateEvidence::Structural => "structural" },
            "location": {"start":c.location.start,"end":c.location.end,"line":c.location.line,"column":c.location.column}
        })).collect::<Vec<_>>());
        let mut entry = json!({"key":r.source_id.value,"source_bytes":r.source_bytes});
        entry["report"] = report;
        Ok(entry)
    }).collect::<Result<Vec<_>, ShareError>>()?;
    let mut batch = json!({});
    batch["entries"] = serde_json::Value::Array(entries);
    let results: cribra::ScanResults<String> =
        serde_json::from_value(batch).map_err(|_| conversion())?;
    project(builder.build(&results, sources.iter().map(String::as_str))?)
}
