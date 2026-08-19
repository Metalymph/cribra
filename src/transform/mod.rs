//! Safe source transformations driven by scan findings.
//!
//! Transformation APIs operate on caller-provided UTF-8 source text together
//! with a [`ScanReport`]. Source text is never retained by
//! [`ScanResults`], reports, findings, or transformation
//! metadata.
//!
//! Transformations validate every finding span before reading or replacing it.
//! The transformation layer is intentionally independent from filesystem,
//! network, terminal, and serialization concerns.

use std::{error::Error, fmt, ops::Range};

use crate::{Finding, ScanReport};

mod pseudonymization;
mod redact;
mod share_bundle;
mod synthesize;
mod template;

pub use pseudonymization::{PseudonymizationOptions, pseudonymize};
pub use redact::{redact, redact_with};
pub use share_bundle::{
    ShareBundle, ShareBundleBuilder, ShareManifest, ShareMode, ShareModeKind, TransformedSource,
};
pub use synthesize::{SynthesisOptions, synthesize};
pub use template::{TemplateOptions, template, template_with};

/// Error returned when a transformation cannot safely apply scan findings to
/// the supplied source.
#[derive(Debug, Clone, Eq, PartialEq)]
#[non_exhaustive]
pub enum TransformError {
    /// A finding span is empty, reversed, or extends outside the source.
    InvalidSpan {
        /// Inclusive start byte offset.
        start: usize,
        /// Exclusive end byte offset.
        end: usize,
        /// Length of the supplied UTF-8 source in bytes.
        source_len: usize,
    },
    /// A finding span does not begin and end on UTF-8 character boundaries.
    InvalidUtf8Boundary {
        /// Inclusive start byte offset.
        start: usize,
        /// Exclusive end byte offset.
        end: usize,
    },
    /// Two spans overlap in a transformation that requires independent spans.
    ///
    /// Redaction can safely merge overlapping spans and therefore does not
    /// return this variant. Other transformations may use it when preserving a
    /// one-finding-to-one-replacement relationship is required.
    OverlappingSpans {
        /// First overlapping half-open byte range.
        first: Range<usize>,
        /// Second overlapping half-open byte range.
        second: Range<usize>,
    },
    /// A share bundle was built without selecting a transformation mode.
    MissingShareMode,
    /// The number of supplied sources does not match the number of scan entries.
    SourceCountMismatch {
        /// Number of sources represented by the scan results.
        expected: usize,
        /// Number of sources supplied to the bundle builder.
        actual: usize,
    },
    /// A supplied source does not have the byte length recorded by its scan entry.
    ///
    /// This catches common source/result mixups without storing or hashing source
    /// content inside [`ScanResults`](crate::ScanResults).
    SourceLengthMismatch {
        /// Zero-based source index in batch order.
        index: usize,
        /// Byte length recorded when the source was scanned.
        expected_bytes: usize,
        /// Byte length of the source supplied to the bundle builder.
        actual_bytes: usize,
    },
}

impl fmt::Display for TransformError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSpan {
                start,
                end,
                source_len,
            } => write!(
                formatter,
                "invalid transform span {start}..{end} for source length {source_len}",
            ),
            Self::InvalidUtf8Boundary { start, end } => write!(
                formatter,
                "transform span {start}..{end} is not aligned to UTF-8 character boundaries",
            ),
            Self::OverlappingSpans { first, second } => write!(
                formatter,
                "transform spans {}..{} and {}..{} overlap",
                first.start, first.end, second.start, second.end,
            ),
            Self::MissingShareMode => {
                formatter.write_str("share bundle transformation mode was not configured")
            }
            Self::SourceCountMismatch { expected, actual } => write!(
                formatter,
                "share bundle expected {expected} sources but received {actual}",
            ),
            Self::SourceLengthMismatch {
                index,
                expected_bytes,
                actual_bytes,
            } => write!(
                formatter,
                "share bundle source {index} has {actual_bytes} bytes but scan results record {expected_bytes}",
            ),
        }
    }
}

impl Error for TransformError {}

#[derive(Debug, Copy, Clone, PartialEq)]
pub(crate) struct TransformSpan<'a> {
    pub(crate) finding: &'a Finding,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl TransformSpan<'_> {
    pub(crate) const fn range(self) -> Range<usize> {
        self.start..self.end
    }
}

/// Validates and sorts report spans without reading matched source values.
pub(crate) fn validated_spans<'a>(
    source: &str,
    report: &'a ScanReport,
) -> Result<Vec<TransformSpan<'a>>, TransformError> {
    let mut spans = Vec::with_capacity(report.len());

    for finding in report {
        let location = finding.location();
        let start = location.start();
        let end = location.end();

        if start >= end || end > source.len() {
            return Err(TransformError::InvalidSpan {
                start,
                end,
                source_len: source.len(),
            });
        }

        if !source.is_char_boundary(start) || !source.is_char_boundary(end) {
            return Err(TransformError::InvalidUtf8Boundary { start, end });
        }

        spans.push(TransformSpan {
            finding,
            start,
            end,
        });
    }

    spans.sort_by(|left, right| {
        left.start
            .cmp(&right.start)
            .then_with(|| left.end.cmp(&right.end))
            .then_with(|| {
                left.finding
                    .rule_id()
                    .as_str()
                    .cmp(right.finding.rule_id().as_str())
            })
    });

    Ok(spans)
}

/// Rejects overlapping spans for transformations that need one independent
/// replacement per finding.
pub(crate) fn ensure_non_overlapping(spans: &[TransformSpan<'_>]) -> Result<(), TransformError> {
    for pair in spans.windows(2) {
        let first = pair[0];
        let second = pair[1];

        if second.start < first.end {
            return Err(TransformError::OverlappingSpans {
                first: first.range(),
                second: second.range(),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Confidence, Location, RuleId, Severity};

    fn finding(id: &str, start: usize, end: usize) -> Finding {
        Finding::new(
            RuleId::from(id),
            Location::from_span(start, end),
            Severity::High,
            Confidence::High,
            None,
        )
    }

    #[test]
    fn validates_and_sorts_spans() {
        let report = ScanReport::new_with_candidates(
            vec![finding("later", 5, 8), finding("first", 0, 2)],
            Vec::new(),
        );

        let spans = validated_spans("abcdefgh", &report).unwrap();

        assert_eq!(spans[0].range(), 0..2);
        assert_eq!(spans[1].range(), 5..8);
    }

    #[test]
    fn rejects_out_of_bounds_span() {
        let report = ScanReport::new_with_candidates(vec![finding("bad", 0, 10)], Vec::new());

        assert_eq!(
            validated_spans("short", &report),
            Err(TransformError::InvalidSpan {
                start: 0,
                end: 10,
                source_len: 5,
            }),
        );
    }

    #[test]
    fn rejects_non_utf8_boundary_span() {
        let report = ScanReport::new_with_candidates(vec![finding("bad", 1, 4)], Vec::new());

        assert_eq!(
            validated_spans("😀x", &report),
            Err(TransformError::InvalidUtf8Boundary { start: 1, end: 4 }),
        );
    }

    #[test]
    fn detects_overlapping_spans_for_strict_transformations() {
        let report = ScanReport::new_with_candidates(
            vec![finding("first", 1, 5), finding("second", 3, 7)],
            Vec::new(),
        );
        let spans = validated_spans("abcdefgh", &report).unwrap();

        assert_eq!(
            ensure_non_overlapping(&spans),
            Err(TransformError::OverlappingSpans {
                first: 1..5,
                second: 3..7,
            }),
        );
    }
}
