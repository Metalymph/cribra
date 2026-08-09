//! One identified source and its scan report.

use crate::ScanReport;

/// Result produced for one identified UTF-8 source.
///
/// `K` is supplied by the caller and can be a file path, editor-buffer ID,
/// database key or any other source identifier. The scanner never interprets
/// the key.
///
/// The entry stores the original UTF-8 byte length so aggregate APIs can report
/// how much text was scanned without retaining the source itself.
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScanEntry<K> {
    key: K,
    source_bytes: usize,
    report: ScanReport,
}

impl<K> ScanEntry<K> {
    pub(crate) const fn new(key: K, source_bytes: usize, report: ScanReport) -> Self {
        Self {
            key,
            source_bytes,
            report,
        }
    }

    /// Returns the caller-supplied source identifier.
    #[must_use]
    pub const fn key(&self) -> &K {
        &self.key
    }

    /// Returns the UTF-8 byte length of the scanned source.
    #[must_use]
    pub const fn source_bytes(&self) -> usize {
        self.source_bytes
    }

    /// Returns the immutable report for this source.
    #[must_use]
    pub const fn report(&self) -> &ScanReport {
        &self.report
    }

    /// Consumes this entry and returns its components.
    #[must_use]
    pub fn into_parts(self) -> (K, usize, ScanReport) {
        (self.key, self.source_bytes, self.report)
    }
}
