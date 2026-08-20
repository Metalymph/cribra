//! Opaque Rust-owned objects used by the native ABI.

use cribra::{ScanReport, Scanner, ScannerBuilder};

/// Opaque native scanner-builder handle.
pub struct CribraBuilder {
    pub(crate) inner: Option<ScannerBuilder>,
}

impl CribraBuilder {
    pub(crate) fn empty() -> Self {
        Self {
            inner: Some(ScannerBuilder::new()),
        }
    }
}

/// Opaque immutable scanner handle.
pub struct CribraScanner {
    pub(crate) inner: Scanner,
}

impl CribraScanner {
    pub(crate) fn new(inner: Scanner) -> Self {
        Self { inner }
    }
}

/// Opaque immutable report handle.
///
/// v0.3.3 deliberately establishes report ownership only. The report read
/// surface is introduced by v0.3.4.
pub struct CribraReport {
    #[allow(dead_code)] // Read by the report traversal surface introduced in v0.3.4.
    pub(crate) inner: ScanReport,
}

impl CribraReport {
    pub(crate) fn new(inner: ScanReport) -> Self {
        Self { inner }
    }
}
