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

pub use ffi::{
    ABI_VERSION_MAJOR, ABI_VERSION_MINOR, ABI_VERSION_PATCH, cribra_abi_version_major,
    cribra_abi_version_minor, cribra_abi_version_patch, cribra_builder_build, cribra_builder_free,
    cribra_builder_new, cribra_report_free, cribra_scanner_free, cribra_scanner_new_current,
    cribra_scanner_scan,
};
pub use handles::{CribraBuilder, CribraReport, CribraScanner};
pub use status::{
    CRIBRA_BUILD_ERROR, CRIBRA_INTERNAL_ERROR, CRIBRA_INVALID_ARGUMENT, CRIBRA_INVALID_UTF8,
    CRIBRA_OK, CribraStatus,
};
