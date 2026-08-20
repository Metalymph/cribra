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
pub const CRIBRA_REMEDIATION_UNKNOWN: CribraRemediation = u32::MAX;

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
pub const CRIBRA_CANDIDATE_KIND_UNKNOWN: CribraCandidateKind = u32::MAX;

/// Stable ABI candidate-evidence representation.
pub type CribraCandidateEvidence = u32;
/// No candidate evidence is present.
pub const CRIBRA_CANDIDATE_EVIDENCE_NONE: CribraCandidateEvidence = 0;
/// Structural evidence without enough semantic context for a finding.
pub const CRIBRA_CANDIDATE_EVIDENCE_STRUCTURAL: CribraCandidateEvidence = 1;
/// Candidate evidence is newer than this ABI projection understands.
pub const CRIBRA_CANDIDATE_EVIDENCE_UNKNOWN: CribraCandidateEvidence = u32::MAX;

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
pub const CRIBRA_DETECTION_MODE_UNKNOWN: CribraDetectionMode = u32::MAX;

/// Stable ABI explanation-kind representation.
pub type CribraExplanationKind = u32;
/// No explanation is present.
pub const CRIBRA_EXPLANATION_NONE: CribraExplanationKind = 0;
/// Explanation for a classified rule-backed finding.
pub const CRIBRA_EXPLANATION_CLASSIFIED: CribraExplanationKind = 1;
/// Explanation for an ambiguous review-only candidate.
pub const CRIBRA_EXPLANATION_AMBIGUOUS: CribraExplanationKind = 2;
/// An explanation kind is newer than this ABI projection understands.
pub const CRIBRA_EXPLANATION_UNKNOWN: CribraExplanationKind = u32::MAX;

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
