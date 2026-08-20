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
