//! Ordered results returned by [`Scanner::scan`](crate::Scanner::scan).

use crate::{Finding, ScanEntry, ScanReport};

/// Ordered results for a batch of identified UTF-8 sources.
///
/// Entries preserve input order. The collection owns caller-supplied keys and
/// reports, but never stores source text.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct ScanResults<K> {
    entries: Vec<ScanEntry<K>>,
}

impl<K> ScanResults<K> {
    pub(crate) const fn new(entries: Vec<ScanEntry<K>>) -> Self {
        Self { entries }
    }

    /// Returns the number of scanned sources.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` when no sources were scanned.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns all entries in input order.
    #[must_use]
    pub fn as_slice(&self) -> &[ScanEntry<K>] {
        &self.entries
    }

    /// Iterates over entries in input order.
    pub fn iter(&self) -> std::slice::Iter<'_, ScanEntry<K>> {
        self.entries.iter()
    }

    /// Iterates over every finding while retaining its source key.
    ///
    /// No finding or key is cloned.
    pub fn findings(&self) -> impl Iterator<Item = (&K, &Finding)> {
        self.entries.iter().flat_map(|entry| {
            entry
                .report()
                .iter()
                .map(move |finding| (entry.key(), finding))
        })
    }

    /// Consumes the collection and returns its entries.
    #[must_use]
    pub fn into_inner(self) -> Vec<ScanEntry<K>> {
        self.entries
    }

    /// Returns the only report when exactly one source was scanned.
    ///
    /// This is a convenience for callers that intentionally submit a
    /// single-element batch while keeping the public scanning model uniform.
    #[must_use]
    pub fn single_report(&self) -> Option<&ScanReport> {
        match self.entries.as_slice() {
            [entry] => Some(entry.report()),
            _ => None,
        }
    }
}

impl<'a, K> IntoIterator for &'a ScanResults<K> {
    type Item = &'a ScanEntry<K>;
    type IntoIter = std::slice::Iter<'a, ScanEntry<K>>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

impl<K> IntoIterator for ScanResults<K> {
    type Item = ScanEntry<K>;
    type IntoIter = std::vec::IntoIter<ScanEntry<K>>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_results_have_no_entries_or_findings() {
        let results = ScanResults::<&str>::default();

        assert!(results.is_empty());
        assert_eq!(results.len(), 0);
        assert_eq!(results.findings().count(), 0);
        assert!(results.single_report().is_none());
    }
}
