//! Borrowed query view over findings contained in [`ScanResults`](crate::ScanResults).

use std::cmp::Ordering;

use crate::{Confidence, Finding, ScanEntry, ScanSort, Severity};

#[derive(Debug, Copy, Clone)]
enum QueryConstraint<T> {
    Exact(T),
    AtLeast(T),
}

/// Borrowed query over findings produced by a batch scan.
///
/// Queries are lazy and allocation-free until sorting or collection is
/// explicitly requested.
#[derive(Debug, Copy, Clone)]
#[must_use = "queries do nothing until they are iterated, inspected, collected, or sorted"]
pub struct ScanQuery<'a, K> {
    entries: &'a [ScanEntry<K>],
    severity: Option<QueryConstraint<Severity>>,
    confidence: Option<QueryConstraint<Confidence>>,
    rule_id: Option<&'a str>,
}

impl<'a, K> ScanQuery<'a, K> {
    pub(crate) const fn new(entries: &'a [ScanEntry<K>]) -> Self {
        Self {
            entries,
            severity: None,
            confidence: None,
            rule_id: None,
        }
    }

    /// Restricts the query to findings with exactly `severity`.
    ///
    /// This replaces any previously configured severity constraint.
    pub const fn severity(mut self, severity: Severity) -> Self {
        self.severity = Some(QueryConstraint::Exact(severity));
        self
    }

    /// Restricts the query to findings whose severity is at least `severity`.
    ///
    /// Severity follows its documented natural ordering:
    /// `Info < Low < Medium < High < Critical`.
    ///
    /// This replaces any previously configured severity constraint.
    pub const fn minimum_severity(mut self, severity: Severity) -> Self {
        self.severity = Some(QueryConstraint::AtLeast(severity));
        self
    }

    /// Restricts the query to findings with exactly `confidence`.
    ///
    /// This replaces any previously configured confidence constraint.
    pub const fn confidence(mut self, confidence: Confidence) -> Self {
        self.confidence = Some(QueryConstraint::Exact(confidence));
        self
    }

    /// Restricts the query to findings whose confidence is at least
    /// `confidence`.
    ///
    /// Confidence follows its documented natural ordering:
    /// `Low < Medium < High`.
    ///
    /// This replaces any previously configured confidence constraint.
    pub const fn minimum_confidence(mut self, confidence: Confidence) -> Self {
        self.confidence = Some(QueryConstraint::AtLeast(confidence));
        self
    }

    /// Restricts the query to findings produced by the exact rule identifier.
    ///
    /// The identifier is borrowed by the query and is compared without
    /// allocation.
    pub const fn rule_id(mut self, rule_id: &'a str) -> Self {
        self.rule_id = Some(rule_id);
        self
    }

    /// Restricts the query to critical findings.
    pub const fn critical(self) -> Self {
        self.severity(Severity::Critical)
    }

    /// Restricts the query to high- or critical-severity findings.
    pub const fn high_priority(self) -> Self {
        self.minimum_severity(Severity::High)
    }

    /// Restricts the query to high-confidence findings.
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
    #[must_use]
    fn matches(&self, finding: &Finding) -> bool {
        if !matches_constraint(finding.severity(), self.severity) {
            return false;
        }

        if !matches_constraint(finding.confidence(), self.confidence) {
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
    pub fn sort(&self, sort: ScanSort) -> SortedScanQuery<'a, K>
    where
        K: Ord,
    {
        let mut findings = self.collect();

        findings.sort_by(|left, right| compare_findings(*left, *right, sort));

        SortedScanQuery::new(findings)
    }

    /// Returns the number of findings selected by this query.
    #[must_use]
    pub fn count(&self) -> usize {
        self.iter().count()
    }

    /// Returns the first finding selected by this query, if any.
    #[must_use]
    pub fn first(&self) -> Option<(&'a K, &'a Finding)> {
        self.iter().next()
    }

    /// Returns the last finding selected by this query, if any.
    ///
    /// The query remains allocation-free. This walks the selected findings to
    /// completion because the underlying filtered iterator is not required to
    /// be double-ended.
    #[must_use]
    pub fn last(&self) -> Option<(&'a K, &'a Finding)> {
        self.iter().last()
    }

    /// Returns `true` when this query selects at least one finding.
    ///
    /// Evaluation short-circuits at the first match.
    #[must_use]
    pub fn has_matches(&self) -> bool {
        self.iter().next().is_some()
    }

    /// Returns `true` when this query selects no findings.
    ///
    /// Evaluation short-circuits at the first match.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.has_matches()
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

fn matches_constraint<T>(value: T, constraint: Option<QueryConstraint<T>>) -> bool
where
    T: Copy + Ord,
{
    match constraint {
        None => true,
        Some(QueryConstraint::Exact(expected)) => value == expected,
        Some(QueryConstraint::AtLeast(minimum)) => value >= minimum,
    }
}

/// Materialized, sorted view of findings selected by a [`ScanQuery`].
///
/// This type owns only the vector of borrowed `(source, finding)` pairs.
/// Source keys and findings remain owned by the originating
/// [`ScanResults`](crate::ScanResults).
#[derive(Debug, Clone)]
#[must_use = "sorted queries should be inspected, iterated, or consumed"]
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

    /// Returns the number of sorted findings.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.findings.len()
    }

    /// Returns `true` when the sorted query contains at least one finding.
    #[must_use]
    pub const fn has_matches(&self) -> bool {
        !self.findings.is_empty()
    }

    /// Returns `true` when the sorted query contains no findings.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.findings.is_empty()
    }

    /// Iterates over sorted `(source, finding)` pairs.
    pub fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = (&'a K, &'a Finding)> + ExactSizeIterator + '_ {
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

    /// Returns the sorted borrowed pairs as a new vector.
    ///
    /// Only references are copied; source keys and findings are not cloned.
    #[must_use]
    pub fn to_vec(&self) -> Vec<(&'a K, &'a Finding)> {
        self.findings.clone()
    }

    /// Consumes the sorted query and returns its borrowed pairs without copying
    /// the vector.
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
            None,
        )
    }

    fn report(rule: &str, severity: Severity) -> ScanReport {
        ScanReport::new_with_candidates(vec![finding(rule, severity, Confidence::High)], Vec::new())
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
            ScanReport::new_with_candidates(
                vec![
                    finding("low", Severity::Low, Confidence::High),
                    finding("high", Severity::High, Confidence::High),
                    finding("critical", Severity::Critical, Confidence::High),
                ],
                Vec::new(),
            ),
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
            ScanReport::new_with_candidates(
                vec![
                    finding("medium", Severity::Medium, Confidence::High),
                    finding("high", Severity::High, Confidence::High),
                    finding("critical", Severity::Critical, Confidence::High),
                ],
                Vec::new(),
            ),
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
            ScanReport::new_with_candidates(
                vec![
                    finding("low", Severity::High, Confidence::Low),
                    finding("medium", Severity::High, Confidence::Medium),
                    finding("high", Severity::High, Confidence::High),
                ],
                Vec::new(),
            ),
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
            ScanReport::new_with_candidates(
                vec![
                    finding("github.token", Severity::Critical, Confidence::High),
                    finding("stripe.secret", Severity::Critical, Confidence::High),
                ],
                Vec::new(),
            ),
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
            ScanReport::new_with_candidates(
                vec![
                    finding("target", Severity::High, Confidence::High),
                    finding("target", Severity::High, Confidence::Medium),
                    finding("target", Severity::Medium, Confidence::High),
                    finding("other", Severity::Critical, Confidence::High),
                ],
                Vec::new(),
            ),
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
            ScanReport::new_with_candidates(
                vec![
                    finding("high", Severity::High, Confidence::High),
                    finding("critical", Severity::Critical, Confidence::High),
                ],
                Vec::new(),
            ),
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
            ScanReport::new_with_candidates(
                vec![
                    finding("medium", Severity::Medium, Confidence::High),
                    finding("high", Severity::High, Confidence::Medium),
                    finding("critical", Severity::Critical, Confidence::High),
                ],
                Vec::new(),
            ),
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
            ScanReport::new_with_candidates(
                vec![
                    finding("zeta", Severity::High, Confidence::High),
                    finding("alpha", Severity::High, Confidence::High),
                    finding("middle", Severity::High, Confidence::High),
                ],
                Vec::new(),
            ),
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
            ScanReport::new_with_candidates(
                vec![
                    finding("medium", Severity::Medium, Confidence::Low),
                    finding("critical", Severity::Critical, Confidence::Medium),
                    finding("high", Severity::High, Confidence::High),
                ],
                Vec::new(),
            ),
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
            ScanReport::new_with_candidates(
                vec![
                    Finding::new(
                        RuleId::from("later"),
                        Location::from_span(8, 9),
                        Severity::High,
                        Confidence::High,
                        None,
                    ),
                    Finding::new(
                        RuleId::from("first"),
                        Location::from_span(1, 2),
                        Severity::High,
                        Confidence::High,
                        None,
                    ),
                ],
                Vec::new(),
            ),
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
            ScanReport::new_with_candidates(
                vec![
                    finding("zeta", Severity::High, Confidence::High),
                    finding("ignored", Severity::Low, Confidence::High),
                    finding("alpha", Severity::Critical, Confidence::High),
                ],
                Vec::new(),
            ),
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

    #[test]
    fn lazy_query_helpers_short_circuit_semantically() {
        let entries = [ScanEntry::new(
            "source",
            2,
            ScanReport::new_with_candidates(
                vec![
                    finding("first", Severity::High, Confidence::High),
                    finding("last", Severity::Critical, Confidence::High),
                ],
                Vec::new(),
            ),
        )];

        let query = ScanQuery::new(&entries).minimum_severity(Severity::High);

        assert!(query.has_matches());
        assert!(!query.is_empty());
        assert_eq!(query.first().unwrap().1.rule_id().as_str(), "first");
        assert_eq!(query.last().unwrap().1.rule_id().as_str(), "last");
        assert_eq!(query.collect().len(), 2);
    }

    #[test]
    fn lazy_query_helpers_handle_no_matches() {
        let entries = [ScanEntry::new(
            "source",
            1,
            report("medium", Severity::Medium),
        )];

        let query = ScanQuery::new(&entries).critical();

        assert!(!query.has_matches());
        assert!(query.is_empty());
        assert!(query.first().is_none());
        assert!(query.last().is_none());
        assert!(query.collect().is_empty());
    }

    #[test]
    fn sorted_query_helpers_match_materialized_state() {
        let entries = [ScanEntry::new(
            "source",
            2,
            ScanReport::new_with_candidates(
                vec![
                    finding("zeta", Severity::High, Confidence::High),
                    finding("alpha", Severity::Critical, Confidence::High),
                ],
                Vec::new(),
            ),
        )];

        let sorted = ScanQuery::new(&entries).sort(ScanSort::RuleId);

        assert_eq!(sorted.len(), 2);
        assert_eq!(sorted.count(), 2);
        assert!(sorted.has_matches());
        assert!(!sorted.is_empty());
        assert_eq!(sorted.to_vec().len(), 2);
        assert_eq!(sorted.first().unwrap().1.rule_id().as_str(), "alpha");
        assert_eq!(sorted.last().unwrap().1.rule_id().as_str(), "zeta");
    }
}
