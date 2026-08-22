//! Stable C-facing value and view representations.

use std::ptr;

/// Borrowed UTF-8 string view.
///
/// The pointed-to bytes are not NUL-terminated. The view does not own memory
/// and is valid only for the lifetime documented by the API that returned it.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct CribraStringView {
    /// Pointer to the first UTF-8 byte.
    pub ptr: *const u8,
    /// Number of bytes in the string.
    pub len: usize,
}

impl Default for CribraStringView {
    fn default() -> Self {
        Self {
            ptr: ptr::null(),
            len: 0,
        }
    }
}

/// Stable ABI severity representation.
pub type CribraSeverity = u32;
/// Informational finding.
pub const CRIBRA_SEVERITY_INFO: CribraSeverity = 0;
/// Low-severity finding.
pub const CRIBRA_SEVERITY_LOW: CribraSeverity = 1;
/// Medium-severity finding.
pub const CRIBRA_SEVERITY_MEDIUM: CribraSeverity = 2;
/// High-severity finding.
pub const CRIBRA_SEVERITY_HIGH: CribraSeverity = 3;
/// Critical finding.
pub const CRIBRA_SEVERITY_CRITICAL: CribraSeverity = 4;

/// Stable ABI confidence representation.
pub type CribraConfidence = u32;
/// Low-confidence finding.
pub const CRIBRA_CONFIDENCE_LOW: CribraConfidence = 0;
/// Medium-confidence finding.
pub const CRIBRA_CONFIDENCE_MEDIUM: CribraConfidence = 1;
/// High-confidence finding.
pub const CRIBRA_CONFIDENCE_HIGH: CribraConfidence = 2;

/// Stable ABI remediation representation.
pub type CribraRemediation = u32;
/// No remediation metadata is attached.
pub const CRIBRA_REMEDIATION_NONE: CribraRemediation = 0;
/// Revoke the credential and issue a replacement.
pub const CRIBRA_REMEDIATION_REVOKE_AND_ROTATE_CREDENTIAL: CribraRemediation = 1;
/// Rotate the credential.
pub const CRIBRA_REMEDIATION_ROTATE_CREDENTIAL: CribraRemediation = 2;
/// Rotate the password or passphrase.
pub const CRIBRA_REMEDIATION_ROTATE_PASSWORD: CribraRemediation = 3;
/// Replace the exposed private key.
pub const CRIBRA_REMEDIATION_REPLACE_PRIVATE_KEY: CribraRemediation = 4;
/// Remove the sensitive value.
pub const CRIBRA_REMEDIATION_REMOVE_SENSITIVE_VALUE: CribraRemediation = 5;
/// Review whether the hash is appropriate to expose.
pub const CRIBRA_REMEDIATION_REVIEW_SENSITIVE_HASH: CribraRemediation = 6;
/// A remediation variant is newer than this ABI projection understands.
pub const CRIBRA_REMEDIATION_UNKNOWN: CribraRemediation = 0xFFFF_FFFF;

/// Borrowed projection of one report-owned finding.
///
/// `rule_id` borrows storage owned by the report. No field exposes the matched
/// source value.
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct CribraFindingView {
    /// Stable rule identifier.
    pub rule_id: CribraStringView,
    /// Zero-based byte offset at which the finding begins.
    pub start: usize,
    /// Zero-based exclusive byte offset at which the finding ends.
    pub end: usize,
    /// One-based source line.
    pub line: usize,
    /// One-based Unicode-scalar column.
    pub column: usize,
    /// ABI-defined severity code.
    pub severity: CribraSeverity,
    /// ABI-defined confidence code.
    pub confidence: CribraConfidence,
    /// ABI-defined remediation code, or [`CRIBRA_REMEDIATION_NONE`].
    pub remediation: CribraRemediation,
}

/// Stable ABI sensitive-candidate kind representation.
pub type CribraCandidateKind = u32;
/// No candidate kind is present.
pub const CRIBRA_CANDIDATE_KIND_NONE: CribraCandidateKind = 0;
/// Structurally plausible recovery or backup code.
pub const CRIBRA_CANDIDATE_KIND_RECOVERY_LIKE_CODE: CribraCandidateKind = 1;
/// A candidate kind is newer than this ABI projection understands.
pub const CRIBRA_CANDIDATE_KIND_UNKNOWN: CribraCandidateKind = 0xFFFF_FFFF;

/// Stable ABI candidate-evidence representation.
pub type CribraCandidateEvidence = u32;
/// No candidate evidence is present.
pub const CRIBRA_CANDIDATE_EVIDENCE_NONE: CribraCandidateEvidence = 0;
/// Structural evidence without enough semantic context for a finding.
pub const CRIBRA_CANDIDATE_EVIDENCE_STRUCTURAL: CribraCandidateEvidence = 1;
/// Candidate evidence is newer than this ABI projection understands.
pub const CRIBRA_CANDIDATE_EVIDENCE_UNKNOWN: CribraCandidateEvidence = 0xFFFF_FFFF;

/// Projection of one report-owned ambiguous sensitive candidate.
///
/// Candidates remain review-only observations. This view deliberately contains
/// no finding severity, finding confidence, remediation, or source value.
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct CribraCandidateView {
    /// ABI-defined candidate family.
    pub kind: CribraCandidateKind,
    /// Zero-based byte offset at which the candidate begins.
    pub start: usize,
    /// Zero-based exclusive byte offset at which the candidate ends.
    pub end: usize,
    /// One-based source line.
    pub line: usize,
    /// One-based Unicode-scalar column.
    pub column: usize,
    /// ABI-defined evidence that caused this value to be surfaced for review.
    pub evidence: CribraCandidateEvidence,
}

/// Stable ABI detection-mode representation.
pub type CribraDetectionMode = u32;
/// No classified detection mode is present.
pub const CRIBRA_DETECTION_MODE_NONE: CribraDetectionMode = 0;
/// The matcher itself is authoritative.
pub const CRIBRA_DETECTION_MODE_MATCHER_ONLY: CribraDetectionMode = 1;
/// Validation depends only on the candidate's own structure.
pub const CRIBRA_DETECTION_MODE_DETERMINISTIC: CribraDetectionMode = 2;
/// Validation also depends on surrounding source context.
pub const CRIBRA_DETECTION_MODE_CONTEXTUAL: CribraDetectionMode = 3;
/// A detection mode is newer than this ABI projection understands.
pub const CRIBRA_DETECTION_MODE_UNKNOWN: CribraDetectionMode = 0xFFFF_FFFF;

/// Stable ABI explanation-kind representation.
pub type CribraExplanationKind = u32;
/// No explanation is present.
pub const CRIBRA_EXPLANATION_NONE: CribraExplanationKind = 0;
/// Explanation for a classified rule-backed finding.
pub const CRIBRA_EXPLANATION_CLASSIFIED: CribraExplanationKind = 1;
/// Explanation for an ambiguous review-only candidate.
pub const CRIBRA_EXPLANATION_AMBIGUOUS: CribraExplanationKind = 2;
/// An explanation kind is newer than this ABI projection understands.
pub const CRIBRA_EXPLANATION_UNKNOWN: CribraExplanationKind = 0xFFFF_FFFF;

/// Stable typed explanation projection.
///
/// Exactly one authority-specific payload is meaningful:
///
/// - classified explanations set `detection_mode`;
/// - ambiguous explanations set `candidate_evidence`.
///
/// The unused payload remains its corresponding `*_NONE` value.
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct CribraExplanationView {
    /// ABI-defined explanation family.
    pub kind: CribraExplanationKind,
    /// Classified detection mode, or [`CRIBRA_DETECTION_MODE_NONE`].
    pub detection_mode: CribraDetectionMode,
    /// Ambiguous candidate evidence, or [`CRIBRA_CANDIDATE_EVIDENCE_NONE`].
    pub candidate_evidence: CribraCandidateEvidence,
}

/// Stable ABI custom-rule family representation.
pub type CribraRuleKind = u32;
/// Exact literal matching.
pub const CRIBRA_RULE_KIND_LITERAL: CribraRuleKind = 0;
/// Token-prefix matching.
pub const CRIBRA_RULE_KIND_PREFIX: CribraRuleKind = 1;
/// Token-suffix matching.
pub const CRIBRA_RULE_KIND_SUFFIX: CribraRuleKind = 2;
/// Full regex-match span projection.
pub const CRIBRA_RULE_KIND_PATTERN: CribraRuleKind = 3;

/// Borrowed configuration used to add one public custom rule.
///
/// `id` and `value` are copied by [`crate::cribra_builder_add_rule`] before the
/// call returns. Internal validators and capture projection are deliberately not
/// represented here.
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct CribraRuleConfig {
    /// ABI-defined public rule family.
    pub kind: CribraRuleKind,
    /// Stable rule identifier.
    pub id: CribraStringView,
    /// Literal, prefix, suffix, or regex source according to `kind`.
    pub value: CribraStringView,
    /// ABI-defined severity assigned to findings.
    pub severity: CribraSeverity,
    /// Optional ABI-defined remediation, or [`CRIBRA_REMEDIATION_NONE`].
    pub remediation: CribraRemediation,
}

/// Configuration for semantic template generation.
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct CribraTemplateConfig {
    /// Placeholder namespace.
    pub namespace: CribraStringView,
    /// `0` disables numbering; `1` enables deterministic per-rule numbering.
    pub numbered: u8,
}

/// Configuration for deterministic keyed pseudonymization.
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct CribraPseudonymizeConfig {
    /// Pointer to exactly 32 caller-owned key bytes.
    pub key: *const u8,
    /// Number of key bytes. Must be exactly 32.
    pub key_len: usize,
    /// Verbatim pseudonym prefix.
    pub prefix: CribraStringView,
    /// Digest bytes requested from the core; the core clamps this to its supported range.
    pub digest_bytes: usize,
}

/// Configuration for deterministic synthetic-value generation.
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct CribraSynthesizeConfig {
    /// Pointer to exactly 32 caller-owned key bytes.
    pub key: *const u8,
    /// Number of key bytes. Must be exactly 32.
    pub key_len: usize,
    /// Marker used by contextual and generic synthetic values.
    pub marker: CribraStringView,
}

/// One borrowed native batch input descriptor.
///
/// Both views are borrowed only for the duration of
/// [`crate::cribra_scanner_scan_batch`]. `key` is copied into Rust-owned result
/// storage; `source` is scanned in place and is never retained.
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct CribraBatchInput {
    /// Caller-defined UTF-8 source identifier.
    pub key: CribraStringView,
    /// UTF-8 source material to scan.
    pub source: CribraStringView,
}

/// Borrowed projection of one ordered batch entry.
///
/// `key` borrows storage owned by the parent [`crate::CribraBatchResults`].
/// No source text is exposed.
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct CribraBatchEntryView {
    /// Caller-defined UTF-8 source identifier copied into batch-owned storage.
    pub key: CribraStringView,
    /// Original source length in bytes.
    pub source_bytes: usize,
    /// Number of classified findings for this source.
    pub finding_count: usize,
    /// Number of ambiguous sensitive candidates for this source.
    pub candidate_count: usize,
}

/// Aggregate metadata for ordered batch results.
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct CribraBatchSummary {
    /// Number of scanned sources.
    pub sources: usize,
    /// Sum of source lengths in bytes.
    pub source_bytes: usize,
    /// Total number of classified findings.
    pub findings: usize,
    /// Total number of ambiguous sensitive candidates.
    pub candidates: usize,
}

/// Stable ABI batch-execution policy.
pub type CribraBatchExecution = u32;
/// Let Cribra select the available execution strategy.
///
/// Without the `cribra-capi/parallel` feature this is serial. With that feature
/// enabled, Cribra may use its parallel batch implementation while preserving
/// ordered, semantically equivalent results.
pub const CRIBRA_BATCH_EXECUTION_AUTO: CribraBatchExecution = 0;
/// Force serial batch execution.
pub const CRIBRA_BATCH_EXECUTION_SERIAL: CribraBatchExecution = 1;

/// Stable ABI share-bundle transformation mode.
pub type CribraShareMode = u32;
/// Conservative `[REDACTED]` replacement.
pub const CRIBRA_SHARE_MODE_REDACT: CribraShareMode = 0;
/// Semantic `<CRIBRA:rule-id>` placeholder generation.
pub const CRIBRA_SHARE_MODE_TEMPLATE: CribraShareMode = 1;
/// Deterministic keyed pseudonymization.
pub const CRIBRA_SHARE_MODE_PSEUDONYMIZE: CribraShareMode = 2;
/// Deterministic synthetic-value generation.
pub const CRIBRA_SHARE_MODE_SYNTHESIZE: CribraShareMode = 3;
/// A manifest mode is newer than this ABI projection understands.
pub const CRIBRA_SHARE_MODE_UNKNOWN: CribraShareMode = 0xFFFF_FFFF;

/// Configuration used to build one share-safe batch.
///
/// `key` is required only for pseudonymization and synthesis and must contain
/// exactly 32 readable bytes. `text` is the pseudonym prefix for
/// pseudonymization and the synthesis marker for synthesis. An empty `text`
/// selects the core default for those modes. `digest_bytes == 0` selects the
/// pseudonymization default; otherwise the value is passed to the core, which
/// clamps it to its supported range.
///
/// Redact and Template ignore the keyed configuration fields.
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct CribraShareBundleConfig {
    /// ABI-defined share transformation mode.
    pub mode: CribraShareMode,
    /// Pointer to caller-owned key bytes for keyed modes.
    pub key: *const u8,
    /// Number of key bytes. Keyed modes require exactly 32.
    pub key_len: usize,
    /// Optional mode-specific UTF-8 configuration.
    pub text: CribraStringView,
    /// Optional pseudonym digest length in bytes; zero selects the core default.
    pub digest_bytes: usize,
}

/// Borrowed projection of one transformed share-bundle source.
///
/// Both `key` and `content` borrow storage owned by the parent
/// [`crate::CribraShareBundle`].
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct CribraShareEntryView {
    /// Copied caller-defined source identifier.
    pub key: CribraStringView,
    /// Share-safe transformed UTF-8 content.
    pub content: CribraStringView,
}

/// Full share-safe projection of [`cribra::ScanSummary`].
///
/// The value contains counters only and never exposes source text, finding
/// values, source keys, or transform keys.
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct CribraScanSummaryView {
    /// Number of scanned sources.
    pub scanned_sources: usize,
    /// Total number of UTF-8 source bytes scanned.
    pub scanned_bytes: usize,
    /// Reports containing at least one finding.
    pub reports_with_findings: usize,
    /// Reports containing no findings.
    pub reports_without_findings: usize,
    /// Total classified findings.
    pub total_findings: usize,
    /// Total ambiguous candidates.
    pub total_candidates: usize,
    /// Reports containing at least one ambiguous candidate.
    pub reports_with_candidates: usize,
    /// Critical findings.
    pub critical: usize,
    /// High-severity findings.
    pub high: usize,
    /// Medium-severity findings.
    pub medium: usize,
    /// Low-severity findings.
    pub low: usize,
    /// Informational findings.
    pub info: usize,
}

/// Share-safe manifest projection.
///
/// Generation time is represented as seconds and nanoseconds since the Unix
/// epoch. No pseudonymization/synthesis key or original source material is
/// represented.
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct CribraShareManifestView {
    /// ABI-defined transformation mode.
    pub mode: CribraShareMode,
    /// Original scan summary copied into the manifest.
    pub summary: CribraScanSummaryView,
    /// Whole seconds since the Unix epoch.
    pub generated_at_secs: u64,
    /// Additional nanoseconds within the generated second.
    pub generated_at_nanos: u32,
}
