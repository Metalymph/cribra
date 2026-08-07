//! Borrowed query view over findings contained in [`ScanResults`](crate::ScanResults).

use std::cmp::Ordering;

use crate::{Confidence, Finding, ScanEntry, ScanSort, Severity};

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
    severity: Option<Severity>,
    minimum_severity: Option<Severity>,
    confidence: Option<Confidence>,
    minimum_confidence: Option<Confidence>,
    rule_id: Option<&'a str>,
}

impl<'a, K> ScanQuery<'a, K> {
    pub(crate) const fn new(entries: &'a [ScanEntry<K>]) -> Self {
        Self {
            entries,
            severity: None,
            minimum_severity: None,
            confidence: None,
            minimum_confidence: None,
            rule_id: None,
        }
    }

    /// Restricts the query to findings with exactly `severity`.
    ///
    /// This replaces any previously configured minimum-severity filter.
    #[must_use]
    pub const fn severity(mut self, severity: Severity) -> Self {
        self.severity = Some(severity);
        self.minimum_severity = None;
        self
    }

    /// Restricts the query to findings whose severity is at least `severity`.
    ///
    /// Severity follows its documented natural ordering:
    /// `Info < Low < Medium < High < Critical`.
    ///
    /// This replaces any previously configured exact-severity filter.
    #[must_use]
    pub const fn minimum_severity(mut self, severity: Severity) -> Self {
        self.minimum_severity = Some(severity);
        self.severity = None;
        self
    }

    /// Restricts the query to findings with exactly `confidence`.
    ///
    /// This replaces any previously configured minimum-confidence filter.
    #[must_use]
    pub const fn confidence(mut self, confidence: Confidence) -> Self {
        self.confidence = Some(confidence);
        self.minimum_confidence = None;
        self
    }

    /// Restricts the query to findings whose confidence is at least
    /// `confidence`.
    ///
    /// Confidence follows its documented natural ordering:
    /// `Low < Medium < High`.
    ///
    /// This replaces any previously configured exact-confidence filter.
    #[must_use]
    pub const fn minimum_confidence(mut self, confidence: Confidence) -> Self {
        self.minimum_confidence = Some(confidence);
        self.confidence = None;
        self
    }

    /// Restricts the query to findings produced by the exact rule identifier.
    ///
    /// The identifier is borrowed by the query and is compared without
    /// allocation.
    #[must_use]
    pub const fn rule_id(mut self, rule_id: &'a str) -> Self {
        self.rule_id = Some(rule_id);
        self
    }

    /// Restricts the query to critical findings.
    #[must_use]
    pub const fn critical(self) -> Self {
        self.severity(Severity::Critical)
    }

    /// Restricts the query to high- or critical-severity findings.
    #[must_use]
    pub const fn high_priority(self) -> Self {
        self.minimum_severity(Severity::High)
    }

    /// Restricts the query to high-confidence findings.
    #[must_use]
    pub const fn high_confidence(self) -> Self {
        self.confidence(Confidence::High)
    }

    /// Iterates over every finding while retaining its source key.
    ///
    /// The iterator is borrowed and allocation-free. Findings are yielded in
    /// source input order and in each report's deterministic finding order.
    pub fn iter(&self) -> impl Iterator<Item = (&'a K, &'a Finding)> + '_ {
        self.entries
            .iter()
            .flat_map(|entry| {
                entry
                    .report()
                    .iter()
                    .map(move |finding| (entry.key(), finding))
            })
            .filter(|(_, finding)| self.matches(finding))
    }

    /// Returns `true` when `finding` satisfies every configured filter.
    fn matches(&self, finding: &Finding) -> bool {
        if let Some(severity) = self.severity
            && finding.severity() != severity
        {
            return false;
        }

        if let Some(minimum) = self.minimum_severity
            && finding.severity() < minimum
        {
            return false;
        }

        if let Some(confidence) = self.confidence
            && finding.confidence() != confidence
        {
            return false;
        }

        if let Some(minimum) = self.minimum_confidence
            && finding.confidence() < minimum
        {
            return false;
        }

        if let Some(rule_id) = self.rule_id
            && finding.rule_id().as_str() != rule_id
        {
            return false;
        }

        true
    }

    /// Materializes and sorts the findings selected by this query.
    ///
    /// Filtering remains lazy until this method is called. Sorting allocates a
    /// vector containing borrowed `(source, finding)` pairs, but it never
    /// clones source keys or findings and never mutates the underlying
    /// [`ScanResults`](crate::ScanResults).
    ///
    /// Source sorting requires `K: Ord`. The same bound is applied to the
    /// complete sorting API so one `sort(ScanSort::...)` method can cover every
    /// supported ordering consistently.
    #[must_use]
    pub fn sort(&self, sort: ScanSort) -> SortedScanQuery<'a, K>
    where
        K: Ord,
    {
        let mut findings = self.collect();

        findings.sort_by(|left, right| compare_findings(*left, *right, sort));

        SortedScanQuery::new(findings)
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

/// Materialized, sorted view of findings selected by a [`ScanQuery`].
///
/// This type owns only the vector of borrowed `(source, finding)` pairs.
/// Source keys and findings remain owned by the originating
/// [`ScanResults`](crate::ScanResults).
#[derive(Debug, Clone)]
pub struct SortedScanQuery<'a, K> {
    findings: Vec<(&'a K, &'a Finding)>,
}

impl<'a, K> SortedScanQuery<'a, K> {
    pub(crate) const fn new(findings: Vec<(&'a K, &'a Finding)>) -> Self {
        Self { findings }
    }

    /// Returns the number of sorted findings.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.findings.len()
    }

    /// Returns `true` when the sorted query contains no findings.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.findings.is_empty()
    }

    /// Iterates over sorted `(source, finding)` pairs.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (&'a K, &'a Finding)> + '_ {
        self.findings.iter().copied()
    }

    /// Returns the first sorted finding, if any.
    #[must_use]
    pub fn first(&self) -> Option<(&'a K, &'a Finding)> {
        self.findings.first().copied()
    }

    /// Returns the last sorted finding, if any.
    #[must_use]
    pub fn last(&self) -> Option<(&'a K, &'a Finding)> {
        self.findings.last().copied()
    }

    /// Returns the sorted findings as a borrowed slice.
    #[must_use]
    pub fn as_slice(&self) -> &[(&'a K, &'a Finding)] {
        &self.findings
    }

    /// Consumes the sorted query and returns its borrowed pairs.
    #[must_use]
    pub fn into_vec(self) -> Vec<(&'a K, &'a Finding)> {
        self.findings
    }
}

impl<'a, K> IntoIterator for SortedScanQuery<'a, K> {
    type Item = (&'a K, &'a Finding);
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.findings.into_iter()
    }
}

impl<'query, 'a, K> IntoIterator for &'query SortedScanQuery<'a, K> {
    type Item = (&'a K, &'a Finding);
    type IntoIter = std::iter::Copied<std::slice::Iter<'query, (&'a K, &'a Finding)>>;

    fn into_iter(self) -> Self::IntoIter {
        self.findings.iter().copied()
    }
}

fn compare_findings<K: Ord>(
    left: (&K, &Finding),
    right: (&K, &Finding),
    sort: ScanSort,
) -> Ordering {
    let primary = match sort {
        ScanSort::RuleId => left.1.rule_id().as_str().cmp(right.1.rule_id().as_str()),
        ScanSort::RuleIdDescending => right.1.rule_id().as_str().cmp(left.1.rule_id().as_str()),
        ScanSort::Source => left.0.cmp(right.0),
        ScanSort::SourceDescending => right.0.cmp(left.0),
        ScanSort::Severity => left.1.severity().cmp(&right.1.severity()),
        ScanSort::SeverityDescending => right.1.severity().cmp(&left.1.severity()),
        ScanSort::Confidence => left.1.confidence().cmp(&right.1.confidence()),
        ScanSort::ConfidenceDescending => right.1.confidence().cmp(&left.1.confidence()),
        ScanSort::Location => left.1.location().cmp(right.1.location()),
        ScanSort::LocationDescending => right.1.location().cmp(left.1.location()),
    };

    if primary != Ordering::Equal {
        return primary;
    }

    // Preserve a deterministic total ordering when multiple findings share the
    // selected sort key. The tie-break chain is intentionally independent from
    // the chosen direction so repeated queries produce a stable result.
    left.0
        .cmp(right.0)
        .then_with(|| left.1.location().cmp(right.1.location()))
        .then_with(|| left.1.rule_id().as_str().cmp(right.1.rule_id().as_str()))
        .then_with(|| left.1.severity().cmp(&right.1.severity()))
        .then_with(|| left.1.confidence().cmp(&right.1.confidence()))
}

#[cfg(test)]
mod tests {
    use crate::{Confidence, Location, RuleId, ScanEntry, ScanReport, Severity};

    use super::*;

    fn finding(rule: &str, severity: Severity, confidence: Confidence) -> Finding {
        Finding::new(
            RuleId::from(rule),
            Location::from_span(0, 1),
            severity,
            confidence,
        )
    }

    fn report(rule: &str, severity: Severity) -> ScanReport {
        ScanReport::new(vec![finding(rule, severity, Confidence::High)])
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
    #[test]
    fn filters_exact_severity() {
        let entries = [ScanEntry::new(
            "source",
            3,
            ScanReport::new(vec![
                finding("low", Severity::Low, Confidence::High),
                finding("high", Severity::High, Confidence::High),
                finding("critical", Severity::Critical, Confidence::High),
            ]),
        )];

        let findings = ScanQuery::new(&entries).severity(Severity::High).collect();

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].1.rule_id().as_str(), "high");
    }

    #[test]
    fn filters_minimum_severity() {
        let entries = [ScanEntry::new(
            "source",
            3,
            ScanReport::new(vec![
                finding("medium", Severity::Medium, Confidence::High),
                finding("high", Severity::High, Confidence::High),
                finding("critical", Severity::Critical, Confidence::High),
            ]),
        )];

        let findings = ScanQuery::new(&entries)
            .minimum_severity(Severity::High)
            .collect();

        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].1.rule_id().as_str(), "high");
        assert_eq!(findings[1].1.rule_id().as_str(), "critical");
    }

    #[test]
    fn filters_confidence_and_minimum_confidence() {
        let entries = [ScanEntry::new(
            "source",
            3,
            ScanReport::new(vec![
                finding("low", Severity::High, Confidence::Low),
                finding("medium", Severity::High, Confidence::Medium),
                finding("high", Severity::High, Confidence::High),
            ]),
        )];

        let exact = ScanQuery::new(&entries)
            .confidence(Confidence::Medium)
            .collect();
        assert_eq!(exact.len(), 1);
        assert_eq!(exact[0].1.rule_id().as_str(), "medium");

        let minimum = ScanQuery::new(&entries)
            .minimum_confidence(Confidence::Medium)
            .collect();
        assert_eq!(minimum.len(), 2);
    }

    #[test]
    fn filters_exact_rule_identifier() {
        let entries = [ScanEntry::new(
            "source",
            2,
            ScanReport::new(vec![
                finding("github.token", Severity::Critical, Confidence::High),
                finding("stripe.secret", Severity::Critical, Confidence::High),
            ]),
        )];

        let findings = ScanQuery::new(&entries).rule_id("stripe.secret").collect();

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].1.rule_id().as_str(), "stripe.secret");
    }

    #[test]
    fn composes_independent_filters() {
        let entries = [ScanEntry::new(
            "source",
            4,
            ScanReport::new(vec![
                finding("target", Severity::High, Confidence::High),
                finding("target", Severity::High, Confidence::Medium),
                finding("target", Severity::Medium, Confidence::High),
                finding("other", Severity::Critical, Confidence::High),
            ]),
        )];

        let findings = ScanQuery::new(&entries)
            .minimum_severity(Severity::High)
            .minimum_confidence(Confidence::High)
            .rule_id("target")
            .collect();

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].1.rule_id().as_str(), "target");
    }

    #[test]
    fn exact_and_minimum_filters_replace_each_other() {
        let entries = [ScanEntry::new(
            "source",
            2,
            ScanReport::new(vec![
                finding("high", Severity::High, Confidence::High),
                finding("critical", Severity::Critical, Confidence::High),
            ]),
        )];

        let findings = ScanQuery::new(&entries)
            .severity(Severity::Critical)
            .minimum_severity(Severity::High)
            .collect();

        assert_eq!(findings.len(), 2);
    }

    #[test]
    fn convenience_filters_match_domain_semantics() {
        let entries = [ScanEntry::new(
            "source",
            3,
            ScanReport::new(vec![
                finding("medium", Severity::Medium, Confidence::High),
                finding("high", Severity::High, Confidence::Medium),
                finding("critical", Severity::Critical, Confidence::High),
            ]),
        )];

        assert_eq!(ScanQuery::new(&entries).critical().count(), 1);
        assert_eq!(ScanQuery::new(&entries).high_priority().count(), 2);
        assert_eq!(ScanQuery::new(&entries).high_confidence().count(), 2);
    }

    #[test]
    fn sorts_by_rule_identifier_in_both_directions() {
        let entries = [ScanEntry::new(
            "source",
            3,
            ScanReport::new(vec![
                finding("zeta", Severity::High, Confidence::High),
                finding("alpha", Severity::High, Confidence::High),
                finding("middle", Severity::High, Confidence::High),
            ]),
        )];

        let ascending = ScanQuery::new(&entries).sort(ScanSort::RuleId);
        let descending = ScanQuery::new(&entries).sort(ScanSort::RuleIdDescending);

        assert_eq!(
            ascending
                .iter()
                .map(|(_, finding)| finding.rule_id().as_str())
                .collect::<Vec<_>>(),
            ["alpha", "middle", "zeta"],
        );
        assert_eq!(
            descending
                .iter()
                .map(|(_, finding)| finding.rule_id().as_str())
                .collect::<Vec<_>>(),
            ["zeta", "middle", "alpha"],
        );
    }

    #[test]
    fn sorts_by_source_key() {
        let entries = [
            ScanEntry::new("z.env", 1, report("z", Severity::High)),
            ScanEntry::new("a.env", 1, report("a", Severity::High)),
            ScanEntry::new("m.env", 1, report("m", Severity::High)),
        ];

        let sorted = ScanQuery::new(&entries).sort(ScanSort::Source);

        assert_eq!(
            sorted.iter().map(|(key, _)| *key).collect::<Vec<_>>(),
            ["a.env", "m.env", "z.env"],
        );
    }

    #[test]
    fn sorts_by_severity_and_confidence() {
        let entries = [ScanEntry::new(
            "source",
            3,
            ScanReport::new(vec![
                finding("medium", Severity::Medium, Confidence::Low),
                finding("critical", Severity::Critical, Confidence::Medium),
                finding("high", Severity::High, Confidence::High),
            ]),
        )];

        let severity = ScanQuery::new(&entries).sort(ScanSort::SeverityDescending);
        assert_eq!(
            severity
                .iter()
                .map(|(_, finding)| finding.severity())
                .collect::<Vec<_>>(),
            [Severity::Critical, Severity::High, Severity::Medium],
        );

        let confidence = ScanQuery::new(&entries).sort(ScanSort::ConfidenceDescending);
        assert_eq!(
            confidence
                .iter()
                .map(|(_, finding)| finding.confidence())
                .collect::<Vec<_>>(),
            [Confidence::High, Confidence::Medium, Confidence::Low],
        );
    }

    #[test]
    fn sorts_by_location() {
        let entries = [ScanEntry::new(
            "source",
            12,
            ScanReport::new(vec![
                Finding::new(
                    RuleId::from("later"),
                    Location::from_span(8, 9),
                    Severity::High,
                    Confidence::High,
                ),
                Finding::new(
                    RuleId::from("first"),
                    Location::from_span(1, 2),
                    Severity::High,
                    Confidence::High,
                ),
            ]),
        )];

        let sorted = ScanQuery::new(&entries).sort(ScanSort::Location);

        assert_eq!(sorted.first().unwrap().1.location().start(), 1);
        assert_eq!(sorted.last().unwrap().1.location().start(), 8);
    }

    #[test]
    fn sorting_happens_after_lazy_filtering() {
        let entries = [ScanEntry::new(
            "source",
            3,
            ScanReport::new(vec![
                finding("zeta", Severity::High, Confidence::High),
                finding("ignored", Severity::Low, Confidence::High),
                finding("alpha", Severity::Critical, Confidence::High),
            ]),
        )];

        let sorted = ScanQuery::new(&entries)
            .minimum_severity(Severity::High)
            .sort(ScanSort::RuleId);

        assert_eq!(sorted.len(), 2);
        assert_eq!(sorted.first().unwrap().1.rule_id().as_str(), "alpha");
        assert_eq!(sorted.last().unwrap().1.rule_id().as_str(), "zeta");
    }

    #[test]
    fn sorted_query_exposes_owned_vector_of_borrowed_pairs() {
        let entries = [ScanEntry::new("source", 1, report("rule", Severity::High))];

        let sorted = ScanQuery::new(&entries).sort(ScanSort::RuleId);

        assert!(!sorted.is_empty());
        assert_eq!(sorted.as_slice().len(), 1);

        let pairs = sorted.into_vec();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].1.rule_id().as_str(), "rule");
    }
}
