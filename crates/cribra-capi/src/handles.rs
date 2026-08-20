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
    pub(crate) inner: ScanReport,
    pub(crate) source_bytes: usize,
}

impl CribraReport {
    pub(crate) fn new(inner: ScanReport, source_bytes: usize) -> Self {
        Self {
            inner,
            source_bytes,
        }
    }
}

/// Opaque Rust-owned transformed UTF-8 output.
///
/// Content is owned by Cribra until [`crate::cribra_output_free`] is called.
pub struct CribraOutput {
    pub(crate) inner: String,
}

impl CribraOutput {
    pub(crate) fn new(inner: String) -> Self {
        Self { inner }
    }
}
