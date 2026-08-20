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
    cribra_abi_version_minor, cribra_abi_version_patch, cribra_builder_build, cribra_builder_free,
    cribra_builder_new, cribra_report_finding_at, cribra_report_finding_count, cribra_report_free,
    cribra_scanner_free, cribra_scanner_new_current, cribra_scanner_scan,
};
pub use handles::{CribraBuilder, CribraReport, CribraScanner};
pub use status::{
    CRIBRA_BUILD_ERROR, CRIBRA_INTERNAL_ERROR, CRIBRA_INVALID_ARGUMENT, CRIBRA_INVALID_UTF8,
    CRIBRA_OK, CRIBRA_OUT_OF_RANGE, CribraStatus,
};
pub use types::{
    CRIBRA_CONFIDENCE_HIGH, CRIBRA_CONFIDENCE_LOW, CRIBRA_CONFIDENCE_MEDIUM,
    CRIBRA_REMEDIATION_NONE, CRIBRA_REMEDIATION_REMOVE_SENSITIVE_VALUE,
    CRIBRA_REMEDIATION_REPLACE_PRIVATE_KEY, CRIBRA_REMEDIATION_REVIEW_SENSITIVE_HASH,
    CRIBRA_REMEDIATION_REVOKE_AND_ROTATE_CREDENTIAL, CRIBRA_REMEDIATION_ROTATE_CREDENTIAL,
    CRIBRA_REMEDIATION_ROTATE_PASSWORD, CRIBRA_REMEDIATION_UNKNOWN, CRIBRA_SEVERITY_CRITICAL,
    CRIBRA_SEVERITY_HIGH, CRIBRA_SEVERITY_INFO, CRIBRA_SEVERITY_LOW, CRIBRA_SEVERITY_MEDIUM,
    CribraConfidence, CribraFindingView, CribraRemediation, CribraSeverity, CribraStringView,
};
