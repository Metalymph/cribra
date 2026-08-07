//! Ordering choices for scan queries.

/// Ordering applied when a scan query is materialized.
///
/// Sorting is intentionally separate from filtering. A query remains
/// allocation-free while filtering and iterating in source order; calling
/// [`ScanQuery::sort`](crate::ScanQuery::sort) materializes only borrowed
/// `(source, finding)` pairs.
///
/// Because source ordering is part of this unified enum, sorting requires the
/// source key type to implement [`Ord`], even when another sort variant is
/// selected. Equal primary keys use deterministic secondary ordering.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum ScanSort {
    /// Sort by rule identifier in ascending lexical order.
    RuleId,
    /// Sort by rule identifier in descending lexical order.
    RuleIdDescending,
    /// Sort by source key in ascending order.
    Source,
    /// Sort by source key in descending order.
    SourceDescending,
    /// Sort by severity in ascending enum order.
    Severity,
    /// Sort by severity in descending enum order.
    SeverityDescending,
    /// Sort by confidence in ascending enum order.
    Confidence,
    /// Sort by confidence in descending enum order.
    ConfidenceDescending,
    /// Sort by source location in ascending order.
    Location,
    /// Sort by source location in descending order.
    LocationDescending,
}
