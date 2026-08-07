//! Borrowed query view over findings contained in [`ScanResults`](crate::ScanResults).

use crate::{Finding, ScanEntry};

/// Borrowed query over findings produced by a batch scan.
///
/// A query never owns source keys, reports or findings. The initial query is
/// allocation-free and iterates findings in their existing deterministic
/// source/report order.
///
/// Filtering and sorting capabilities are layered onto this type without
/// mutating the underlying [`ScanResults`](crate::ScanResults).
#[derive(Debug, Copy, Clone)]
pub struct ScanQuery<'a, K> {
    entries: &'a [ScanEntry<K>],
}

impl<'a, K> ScanQuery<'a, K> {
    pub(crate) const fn new(entries: &'a [ScanEntry<K>]) -> Self {
        Self { entries }
    }

    /// Iterates over every finding while retaining its source key.
    ///
    /// The iterator is borrowed and allocation-free. Findings are yielded in
    /// source input order and in each report's deterministic finding order.
    pub fn iter(&self) -> impl Iterator<Item = (&'a K, &'a Finding)> + '_ {
        self.entries.iter().flat_map(|entry| {
            entry
                .report()
                .iter()
                .map(move |finding| (entry.key(), finding))
        })
    }

    /// Returns the number of findings selected by this query.
    ///
    /// In this initial unfiltered query stage, this is the total number of
    /// findings in the referenced batch.
    #[must_use]
    pub fn count(&self) -> usize {
        self.iter().count()
    }

    /// Returns the first finding selected by this query, if any.
    #[must_use]
    pub fn first(&self) -> Option<(&'a K, &'a Finding)> {
        self.iter().next()
    }

    /// Materializes the selected findings as borrowed `(source, finding)`
    /// pairs.
    ///
    /// Neither source keys nor findings are cloned.
    #[must_use]
    pub fn collect(&self) -> Vec<(&'a K, &'a Finding)> {
        self.iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::{Confidence, Location, RuleId, ScanEntry, ScanReport, Severity};

    use super::*;

    fn report(rule: &str, severity: Severity) -> ScanReport {
        ScanReport::new(vec![Finding::new(
            RuleId::from(rule),
            Location::from_span(0, 1),
            severity,
            Confidence::High,
        )])
    }

    #[test]
    fn iterates_all_findings_without_cloning() {
        let entries = [
            ScanEntry::new("a", 1, report("one", Severity::High)),
            ScanEntry::new("b", 1, report("two", Severity::Medium)),
        ];
        let query = ScanQuery::new(&entries);

        let findings = query.collect();

        assert_eq!(findings.len(), 2);
        assert_eq!(*findings[0].0, "a");
        assert_eq!(findings[0].1.rule_id().as_str(), "one");
        assert_eq!(*findings[1].0, "b");
        assert_eq!(findings[1].1.rule_id().as_str(), "two");
    }

    #[test]
    fn exposes_count_and_first_helpers() {
        let entries = [ScanEntry::new(
            "source",
            1,
            report("rule", Severity::Critical),
        )];
        let query = ScanQuery::new(&entries);

        assert_eq!(query.count(), 1);

        let (key, finding) = query.first().expect("one finding should exist");
        assert_eq!(*key, "source");
        assert_eq!(finding.rule_id().as_str(), "rule");
    }

    #[test]
    fn empty_query_has_no_findings() {
        let entries: [ScanEntry<&str>; 0] = [];
        let query = ScanQuery::new(&entries);

        assert_eq!(query.count(), 0);
        assert!(query.first().is_none());
        assert!(query.collect().is_empty());
    }
}
