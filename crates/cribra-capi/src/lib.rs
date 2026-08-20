//! Native C ABI adapter for Cribra.
//!
//! This crate is the dedicated native interoperability boundary around the
//! Rust-native `cribra` core. The core remains free of FFI concerns and keeps
//! its `#![forbid(unsafe_code)]` invariant.
//!
//! The ABI is experimental but compatibility-conscious. Cribra crate SemVer and
//! the native ABI protocol version are intentionally independent.

#![warn(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]

mod ffi;
mod handles;
mod status;
mod types;

pub use ffi::{
    ABI_VERSION_MAJOR, ABI_VERSION_MINOR, ABI_VERSION_PATCH, cribra_abi_version_major,
    cribra_abi_version_minor, cribra_abi_version_patch, cribra_builder_add_current_builtins,
    cribra_builder_add_rule, cribra_builder_build, cribra_builder_free, cribra_builder_new,
    cribra_report_candidate_at, cribra_report_candidate_count, cribra_report_explain_candidate,
    cribra_report_finding_at, cribra_report_finding_count, cribra_report_free,
    cribra_scanner_explain_finding, cribra_scanner_free, cribra_scanner_new_current,
    cribra_scanner_scan,
};
pub use handles::{CribraBuilder, CribraReport, CribraScanner};
pub use status::{
    CRIBRA_BUILD_ERROR, CRIBRA_INTERNAL_ERROR, CRIBRA_INVALID_ARGUMENT, CRIBRA_INVALID_UTF8,
    CRIBRA_OK, CRIBRA_OUT_OF_RANGE, CribraStatus,
};
pub use types::{
    CRIBRA_CANDIDATE_EVIDENCE_NONE, CRIBRA_CANDIDATE_EVIDENCE_STRUCTURAL,
    CRIBRA_CANDIDATE_EVIDENCE_UNKNOWN, CRIBRA_CANDIDATE_KIND_NONE,
    CRIBRA_CANDIDATE_KIND_RECOVERY_LIKE_CODE, CRIBRA_CANDIDATE_KIND_UNKNOWN,
    CRIBRA_CONFIDENCE_HIGH, CRIBRA_CONFIDENCE_LOW, CRIBRA_CONFIDENCE_MEDIUM,
    CRIBRA_DETECTION_MODE_CONTEXTUAL, CRIBRA_DETECTION_MODE_DETERMINISTIC,
    CRIBRA_DETECTION_MODE_MATCHER_ONLY, CRIBRA_DETECTION_MODE_NONE, CRIBRA_DETECTION_MODE_UNKNOWN,
    CRIBRA_EXPLANATION_AMBIGUOUS, CRIBRA_EXPLANATION_CLASSIFIED, CRIBRA_EXPLANATION_NONE,
    CRIBRA_EXPLANATION_UNKNOWN, CRIBRA_REMEDIATION_NONE, CRIBRA_REMEDIATION_REMOVE_SENSITIVE_VALUE,
    CRIBRA_REMEDIATION_REPLACE_PRIVATE_KEY, CRIBRA_REMEDIATION_REVIEW_SENSITIVE_HASH,
    CRIBRA_REMEDIATION_REVOKE_AND_ROTATE_CREDENTIAL, CRIBRA_REMEDIATION_ROTATE_CREDENTIAL,
    CRIBRA_REMEDIATION_ROTATE_PASSWORD, CRIBRA_REMEDIATION_UNKNOWN, CRIBRA_RULE_KIND_LITERAL,
    CRIBRA_RULE_KIND_PATTERN, CRIBRA_RULE_KIND_PREFIX, CRIBRA_RULE_KIND_SUFFIX,
    CRIBRA_SEVERITY_CRITICAL, CRIBRA_SEVERITY_HIGH, CRIBRA_SEVERITY_INFO, CRIBRA_SEVERITY_LOW,
    CRIBRA_SEVERITY_MEDIUM, CribraCandidateEvidence, CribraCandidateKind, CribraCandidateView,
    CribraConfidence, CribraDetectionMode, CribraExplanationKind, CribraExplanationView,
    CribraFindingView, CribraRemediation, CribraRuleConfig, CribraRuleKind, CribraSeverity,
    CribraStringView,
};
