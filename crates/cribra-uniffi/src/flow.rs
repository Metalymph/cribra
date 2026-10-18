//! Explicit-finding Flow transformations delegated entirely to Core.

use crate::{Confidence, Finding};
use serde_json::json;

/// Replacement selection for Core redaction.
#[derive(Debug, Default, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RedactOptions {
    /// None uses Core's `[REDACTED]`; Some is passed verbatim, including empty text.
    pub replacement: Option<String>,
}

/// Options passed to Core's semantic template generator without normalization.
#[derive(Debug, Default, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TemplateOptions {
    /// None uses Core's default namespace; Some is interpreted by Core unchanged.
    pub namespace: Option<String>,
    /// Core's deterministic per-rule occurrence numbering in sorted span order.
    pub numbered: bool,
}

/// Caller-supplied options for Core's deterministic keyed pseudonymization.
/// The key is input-only and has no default, generation or persistence path.
#[derive(Clone, uniffi::Record)]
pub struct PseudonymizationOptions {
    /// Exactly 32 explicit bytes (`Data` in Swift); never copied into errors/results.
    pub key: Vec<u8>,
    /// None selects Core's default; Some is output verbatim, including empty text.
    pub prefix: Option<String>,
    /// None selects Core's default. Core clamps configured lengths to 8..=32.
    pub digest_bytes: Option<u64>,
}

impl std::fmt::Debug for PseudonymizationOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PseudonymizationOptions")
            .field("key", &"<redacted>")
            .field("prefix", &self.prefix)
            .field("digest_bytes", &self.digest_bytes)
            .finish()
    }
}

/// Caller-supplied options for Core's deterministic synthetic-value generation.
#[derive(Clone, uniffi::Record)]
pub struct SynthesisOptions {
    /// Exactly 32 explicit bytes (`Data` in Swift); never copied into errors/results.
    pub key: Vec<u8>,
    /// None selects Core's default; Core owns marker normalization and fallback.
    pub marker: Option<String>,
}

impl std::fmt::Debug for SynthesisOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SynthesisOptions")
            .field("key", &"<redacted>")
            .field("marker", &self.marker)
            .finish()
    }
}

/// Owned transformed content only; no hidden original-source copy is retained.
///
/// As in Core, text outside supplied findings is preserved, including all text
/// when no findings are supplied. This does not imply that the content is secret-free.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TransformResult {
    /// Complete output of the requested Core transformation.
    pub content: String,
}

/// Zero-based half-open UTF-8 byte range carried in an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct ByteSpan {
    /// Inclusive start byte offset.
    pub start: u64,
    /// Exclusive end byte offset.
    pub end: u64,
}

/// Stable symbolic error codes; diagnostic text and numeric discriminants are not stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum TransformErrorCode {
    /// Core rejected an empty, reversed or out-of-source span.
    InvalidSpan,
    /// Core rejected a span endpoint inside a UTF-8 scalar.
    InvalidUtf8Boundary,
    /// Core rejected overlapping or duplicate spans in a strict transformation.
    OverlappingSpans,
    /// Core's public serde model could not represent supplied finding metadata.
    InputConversion,
    /// Core returned an error outside the exposed transformation contract.
    UnknownCoreError,
    /// The caller's key does not contain exactly 32 bytes; detail gives length only.
    InvalidKeyLength,
}

/// Structured failure, without source text or a partial transformation result.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Error)]
pub enum TransformError {
    /// Core span error or explicit DTO-to-Core representation failure.
    Rejected {
        /// Stable symbolic reason.
        code: TransformErrorCode,
        /// Invalid span, or first overlapping span, when supplied by Core.
        span: Option<ByteSpan>,
        /// Second overlapping span, when supplied by Core.
        other_span: Option<ByteSpan>,
        /// UTF-8 source byte length, when supplied by Core.
        source_len: Option<u64>,
        /// Original Core diagnostic or serde conversion detail; not for parsing.
        detail: String,
    },
}

impl std::fmt::Display for TransformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self::Rejected { detail, .. } = self;
        f.write_str(detail)
    }
}
impl std::error::Error for TransformError {}

impl From<cribra::transform::TransformError> for TransformError {
    fn from(error: cribra::transform::TransformError) -> Self {
        use cribra::transform::TransformError as Core;
        let bytes = |start: usize, end: usize| {
            Some(ByteSpan {
                start: start as u64,
                end: end as u64,
            })
        };
        let (code, span, other_span, source_len) = match &error {
            Core::InvalidSpan {
                start,
                end,
                source_len,
            } => (
                TransformErrorCode::InvalidSpan,
                bytes(*start, *end),
                None,
                Some(*source_len as u64),
            ),
            Core::InvalidUtf8Boundary { start, end } => (
                TransformErrorCode::InvalidUtf8Boundary,
                bytes(*start, *end),
                None,
                None,
            ),
            Core::OverlappingSpans { first, second } => (
                TransformErrorCode::OverlappingSpans,
                bytes(first.start, first.end),
                bytes(second.start, second.end),
                None,
            ),
            _ => (TransformErrorCode::UnknownCoreError, None, None, None),
        };
        Self::Rejected {
            code,
            span,
            other_span,
            source_len,
            detail: error.to_string(),
        }
    }
}

/// Reconstitutes Core's public serde model without validation or scanning.
/// Input order, byte offsets, IDs and metadata are transferred unchanged.
pub(crate) fn core_report(findings: Vec<Finding>) -> Result<cribra::ScanReport, TransformError> {
    let findings: Vec<_> = findings
        .into_iter()
        .map(|finding| {
            let confidence = match finding.confidence {
                Confidence::Low => "low",
                Confidence::Medium => "medium",
                Confidence::High => "high",
            };
            json!({
                "rule_id": finding.rule_id.value,
                "location": { "start": finding.location.start, "end": finding.location.end,
                    "line": finding.location.line, "column": finding.location.column },
                "severity": cribra::Severity::from(finding.severity).to_string(),
                "confidence": confidence,
            })
        })
        .collect();
    // json!(findings) serializes a borrowed Vec<Value>, cloning its entire tree.
    // Move that tree into the wrapper; the same Core deserializer still runs.
    let mut report = json!({"candidates":[]});
    report["findings"] = serde_json::Value::Array(findings);
    serde_json::from_value(report).map_err(|error| TransformError::Rejected {
        code: TransformErrorCode::InputConversion,
        span: None,
        other_span: None,
        source_len: None,
        detail: error.to_string(),
    })
}

/// Applies Core Redact to exactly the supplied findings, without rescanning.
/// Core validates spans and merges overlaps/duplicates; adjacent spans stay separate.
#[uniffi::export]
pub fn redact(
    source: String,
    findings: Vec<Finding>,
    options: RedactOptions,
) -> Result<TransformResult, TransformError> {
    let report = core_report(findings)?;
    let content = match options.replacement {
        Some(replacement) => {
            cribra::transform::redact_with(&source, &report, &cribra::Redaction::new(replacement))?
        }
        None => cribra::transform::redact(&source, &report)?,
    };
    Ok(TransformResult { content })
}

/// Applies Core Template to exactly the supplied findings, without rescanning.
/// Core owns validation, ordering, overlap rejection and placeholder normalization.
#[uniffi::export]
pub fn template(
    source: String,
    findings: Vec<Finding>,
    options: TemplateOptions,
) -> Result<TransformResult, TransformError> {
    let report = core_report(findings)?;
    let mut core_options = cribra::transform::TemplateOptions::default().numbered(options.numbered);
    if let Some(namespace) = options.namespace {
        core_options = core_options.namespace(namespace);
    }
    Ok(TransformResult {
        content: cribra::transform::template_with(&source, &report, &core_options)?,
    })
}

fn explicit_key(key: Vec<u8>) -> Result<[u8; 32], TransformError> {
    key.try_into()
        .map_err(|key: Vec<u8>| TransformError::Rejected {
            code: TransformErrorCode::InvalidKeyLength,
            span: None,
            other_span: None,
            source_len: None,
            detail: format!(
                "transformation key must contain exactly 32 bytes; received {}",
                key.len()
            ),
        })
}

/// Applies only Core pseudonymization to the explicit findings and explicit key.
/// Core owns hashing, digest bounds, span validation and overlap rejection.
#[uniffi::export]
pub fn pseudonymize(
    source: String,
    findings: Vec<Finding>,
    options: PseudonymizationOptions,
) -> Result<TransformResult, TransformError> {
    let core_options = options.into_core()?;
    let report = core_report(findings)?;
    Ok(TransformResult {
        content: cribra::transform::pseudonymize(&source, &report, &core_options)?,
    })
}

/// Applies only Core synthesis to the explicit findings and explicit key.
/// Core owns provider-specific generation, marker normalization and determinism.
#[uniffi::export]
pub fn synthesize(
    source: String,
    findings: Vec<Finding>,
    options: SynthesisOptions,
) -> Result<TransformResult, TransformError> {
    let core_options = options.into_core()?;
    let report = core_report(findings)?;
    Ok(TransformResult {
        content: cribra::transform::synthesize(&source, &report, &core_options)?,
    })
}

impl PseudonymizationOptions {
    pub(crate) fn into_core(
        self,
    ) -> Result<cribra::transform::PseudonymizationOptions, TransformError> {
        let mut core_options =
            cribra::transform::PseudonymizationOptions::new(explicit_key(self.key)?);
        if let Some(prefix) = self.prefix {
            core_options = core_options.prefix(prefix);
        }
        if let Some(bytes) = self.digest_bytes {
            let bytes = usize::try_from(bytes).map_err(|_| TransformError::Rejected {
                code: TransformErrorCode::InputConversion,
                span: None,
                other_span: None,
                source_len: None,
                detail: "digest byte length cannot be represented on this target".into(),
            })?;
            core_options = core_options.digest_bytes(bytes);
        }
        Ok(core_options)
    }
}

impl SynthesisOptions {
    pub(crate) fn into_core(self) -> Result<cribra::transform::SynthesisOptions, TransformError> {
        let mut core_options = cribra::transform::SynthesisOptions::new(explicit_key(self.key)?);
        if let Some(marker) = self.marker {
            core_options = core_options.marker(marker);
        }
        Ok(core_options)
    }
}
