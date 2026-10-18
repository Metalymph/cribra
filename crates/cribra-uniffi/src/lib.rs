//! Shared, owned UniFFI data model for Cribra.
//!
//! Alongside the DTOs, this foundation exports a version query, an experimental
//! smoke scan and a reusable, configurable Scanner with `core`.
//! Core findings are copied without retaining source text in returned data.

#![forbid(unsafe_code)]

uniffi::setup_scaffolding!();

mod dto;

#[cfg(feature = "core")]
mod conversions;

#[cfg(feature = "core")]
mod configuration;
#[cfg(feature = "flow")]
mod flow;
#[cfg(feature = "core")]
mod incremental;
#[cfg(feature = "core")]
mod scanner;
#[cfg(feature = "flow")]
mod share;
#[cfg(feature = "core")]
mod stream;

#[cfg(feature = "core")]
pub use configuration::{
    BuildErrorCode, DetectionMode, Remediation, RemediationMetadata, RuleKind, RuleMetadata,
    RuleSpec, ScannerBuildError, ScannerConfig,
};

pub use dto::{
    Confidence, Finding, Location, RuleId, ScanReport, ScanResults, ScanSource, ScanSummary,
    Severity, SourceId,
};
#[cfg(feature = "flow")]
pub use flow::{
    ByteSpan, PseudonymizationOptions, RedactOptions, SynthesisOptions, TemplateOptions,
    TransformError, TransformErrorCode, TransformResult, pseudonymize, redact, synthesize,
    template,
};
#[cfg(feature = "core")]
pub use incremental::{
    IncrementalError, IncrementalErrorCode, IncrementalLimits, IncrementalSession,
    IncrementalSnapshot, IncrementalState,
};
#[cfg(feature = "core")]
pub use scanner::Scanner;
#[cfg(feature = "core")]
pub use stream::{StreamError, StreamErrorCode, StreamLimits, StreamSession, StreamState};

/// Returns the Cribra Core package version embedded at build time.
#[uniffi::export]
pub fn cribra_version() -> String {
    env!("CRIBRA_CORE_VERSION").to_owned()
}

/// Scans one UTF-8 string with the Core's default built-in rules.
///
/// This experimental NB-1B.1 smoke entry point is not a definitive Scanner API.
/// It returns only classified findings, in Core order, using the existing DTOs.
#[cfg(feature = "core")]
#[uniffi::export]
pub fn smoke_scan_text(source: String) -> Vec<Finding> {
    let scanner = cribra::Scanner::default();
    let results = scanner.scan([("smoke", source.as_str())]);
    results
        .single_report()
        .expect("one input produces one report")
        .findings()
        .iter()
        .map(Finding::from)
        .collect()
}

#[cfg(feature = "flow")]
pub use share::{
    ShareBundle, ShareBundleOptions, ShareCandidate, ShareCandidateEvidence, ShareCandidateKind,
    ShareError, ShareErrorCode, ShareManifest, ShareMode, ShareModeKind, ShareReport,
    ShareTimestamp, TransformedSource, build, scan_and_build,
};
