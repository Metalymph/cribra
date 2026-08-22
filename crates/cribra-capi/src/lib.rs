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

mod error;
mod ffi;
mod handles;
mod status;
mod types;

pub use error::{CribraError, cribra_error_free, cribra_error_message, cribra_error_status};
pub use ffi::{
    ABI_VERSION_MAJOR, ABI_VERSION_MINOR, ABI_VERSION_PATCH, cribra_abi_version_major,
    cribra_abi_version_minor, cribra_abi_version_patch, cribra_batch_results_candidate_at,
    cribra_batch_results_count, cribra_batch_results_entry_at, cribra_batch_results_finding_at,
    cribra_batch_results_free, cribra_batch_results_summary, cribra_builder_add_current_builtins,
    cribra_builder_add_rule, cribra_builder_build, cribra_builder_free, cribra_builder_new,
    cribra_output_free, cribra_output_view, cribra_report_candidate_at,
    cribra_report_candidate_count, cribra_report_explain_candidate, cribra_report_finding_at,
    cribra_report_finding_count, cribra_report_free, cribra_scanner_explain_finding,
    cribra_scanner_free, cribra_scanner_new_current, cribra_scanner_scan,
    cribra_scanner_scan_batch, cribra_share_bundle_build, cribra_share_bundle_count,
    cribra_share_bundle_entry_at, cribra_share_bundle_free, cribra_share_bundle_manifest,
    cribra_transform_pseudonymize, cribra_transform_redact, cribra_transform_redact_with,
    cribra_transform_synthesize, cribra_transform_template, cribra_transform_template_with,
};
pub use handles::{
    CribraBatchResults, CribraBuilder, CribraOutput, CribraReport, CribraScanner, CribraShareBundle,
};
pub use status::{
    CRIBRA_BUILD_ERROR, CRIBRA_INTERNAL_ERROR, CRIBRA_INVALID_ARGUMENT, CRIBRA_INVALID_UTF8,
    CRIBRA_OK, CRIBRA_OUT_OF_RANGE, CRIBRA_RULE_ERROR, CRIBRA_TRANSFORM_ERROR, CribraStatus,
};
pub use types::{
    CRIBRA_BATCH_EXECUTION_AUTO, CRIBRA_BATCH_EXECUTION_SERIAL, CRIBRA_CANDIDATE_EVIDENCE_NONE,
    CRIBRA_CANDIDATE_EVIDENCE_STRUCTURAL, CRIBRA_CANDIDATE_EVIDENCE_UNKNOWN,
    CRIBRA_CANDIDATE_KIND_NONE, CRIBRA_CANDIDATE_KIND_RECOVERY_LIKE_CODE,
    CRIBRA_CANDIDATE_KIND_UNKNOWN, CRIBRA_CONFIDENCE_HIGH, CRIBRA_CONFIDENCE_LOW,
    CRIBRA_CONFIDENCE_MEDIUM, CRIBRA_DETECTION_MODE_CONTEXTUAL,
    CRIBRA_DETECTION_MODE_DETERMINISTIC, CRIBRA_DETECTION_MODE_MATCHER_ONLY,
    CRIBRA_DETECTION_MODE_NONE, CRIBRA_DETECTION_MODE_UNKNOWN, CRIBRA_EXPLANATION_AMBIGUOUS,
    CRIBRA_EXPLANATION_CLASSIFIED, CRIBRA_EXPLANATION_NONE, CRIBRA_EXPLANATION_UNKNOWN,
    CRIBRA_REMEDIATION_NONE, CRIBRA_REMEDIATION_REMOVE_SENSITIVE_VALUE,
    CRIBRA_REMEDIATION_REPLACE_PRIVATE_KEY, CRIBRA_REMEDIATION_REVIEW_SENSITIVE_HASH,
    CRIBRA_REMEDIATION_REVOKE_AND_ROTATE_CREDENTIAL, CRIBRA_REMEDIATION_ROTATE_CREDENTIAL,
    CRIBRA_REMEDIATION_ROTATE_PASSWORD, CRIBRA_REMEDIATION_UNKNOWN, CRIBRA_RULE_KIND_LITERAL,
    CRIBRA_RULE_KIND_PATTERN, CRIBRA_RULE_KIND_PREFIX, CRIBRA_RULE_KIND_SUFFIX,
    CRIBRA_SEVERITY_CRITICAL, CRIBRA_SEVERITY_HIGH, CRIBRA_SEVERITY_INFO, CRIBRA_SEVERITY_LOW,
    CRIBRA_SEVERITY_MEDIUM, CRIBRA_SHARE_MODE_PSEUDONYMIZE, CRIBRA_SHARE_MODE_REDACT,
    CRIBRA_SHARE_MODE_SYNTHESIZE, CRIBRA_SHARE_MODE_TEMPLATE, CRIBRA_SHARE_MODE_UNKNOWN,
    CribraBatchEntryView, CribraBatchExecution, CribraBatchInput, CribraBatchSummary,
    CribraCandidateEvidence, CribraCandidateKind, CribraCandidateView, CribraConfidence,
    CribraDetectionMode, CribraExplanationKind, CribraExplanationView, CribraFindingView,
    CribraPseudonymizeConfig, CribraRemediation, CribraRuleConfig, CribraRuleKind,
    CribraScanSummaryView, CribraSeverity, CribraShareBundleConfig, CribraShareEntryView,
    CribraShareManifestView, CribraShareMode, CribraStringView, CribraSynthesizeConfig,
    CribraTemplateConfig,
};
