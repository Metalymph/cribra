//! Safe redaction of detected source spans.
//!
//! Redaction replaces every detected span with caller-selected replacement
//! text. Overlapping or duplicate findings are merged before output is built,
//! preventing any portion of an overlapping detected region from leaking.

use std::ops::Range;

use crate::{Redaction, ScanReport};

use super::{TransformError, TransformSpan, validated_spans};

/// Redacts all findings in `report` using the standard `[REDACTED]` marker.
///
/// The returned string contains the original source outside detected spans.
/// The source and matched values are never stored in the report or in
/// transformation metadata.
///
/// # Errors
///
/// Returns [`TransformError`] when a finding contains an invalid or non-UTF-8
/// aligned byte span for `source`.
///
/// # Examples
///
/// ```
/// use cribra::{Rule, Scanner, Severity, transform::redact};
///
/// let scanner = Scanner::builder()
///     .rule(Rule::literal("secret", "SECRET", Severity::High))
///     .build()?;
///
/// let source = "TOKEN=SECRET";
/// let results = scanner.scan([("memory", source)]);
/// let report = results.single_report().expect("one report");
///
/// assert_eq!(redact(source, report)?, "TOKEN=[REDACTED]");
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn redact(source: &str, report: &ScanReport) -> Result<String, TransformError> {
    redact_with(source, report, &Redaction::hidden())
}

/// Redacts all findings in `report` using `redaction` as replacement text.
///
/// Duplicate and partially overlapping findings are replaced by one marker for
/// the complete union of their spans. Adjacent non-overlapping findings remain
/// separate replacements.
///
/// # Errors
///
/// Returns [`TransformError`] when a finding contains an invalid or non-UTF-8
/// aligned byte span for `source`.
pub fn redact_with(
    source: &str,
    report: &ScanReport,
    redaction: &Redaction,
) -> Result<String, TransformError> {
    let spans = validated_spans(source, report)?;

    if spans.is_empty() {
        return Ok(source.to_owned());
    }

    let ranges = merged_ranges(&spans);
    let replacement = redaction.as_str();

    let removed_bytes = ranges
        .iter()
        .map(|range| range.end - range.start)
        .sum::<usize>();
    let replacement_bytes = replacement.len().saturating_mul(ranges.len());

    let mut output = String::with_capacity(
        source
            .len()
            .saturating_sub(removed_bytes)
            .saturating_add(replacement_bytes),
    );

    let mut cursor = 0;

    for range in ranges {
        output.push_str(&source[cursor..range.start]);
        output.push_str(replacement);
        cursor = range.end;
    }

    output.push_str(&source[cursor..]);

    Ok(output)
}

fn merged_ranges(spans: &[TransformSpan<'_>]) -> Vec<Range<usize>> {
    let mut ranges = Vec::<Range<usize>>::with_capacity(spans.len());

    for span in spans {
        match ranges.last_mut() {
            Some(previous) if span.start < previous.end => {
                previous.end = previous.end.max(span.end);
            }
            _ => ranges.push(span.range()),
        }
    }

    ranges
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Confidence, Finding, Location, RuleId, Severity};

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
    fn redacts_one_span_with_safe_default() {
        let report = ScanReport::new_with_candidates(vec![finding("secret", 6, 12)], Vec::new());

        assert_eq!(redact("TOKEN=SECRET", &report).unwrap(), "TOKEN=[REDACTED]",);
    }

    #[test]
    fn supports_custom_replacement() {
        let report = ScanReport::new_with_candidates(vec![finding("secret", 6, 12)], Vec::new());
        let replacement = Redaction::new("***");

        assert_eq!(
            redact_with("TOKEN=SECRET", &report, &replacement).unwrap(),
            "TOKEN=***",
        );
    }

    #[test]
    fn empty_report_preserves_source_exactly() {
        let source = "ordinary 😀 UTF-8";

        assert_eq!(redact(source, &ScanReport::default()).unwrap(), source,);
    }

    #[test]
    fn redacts_multiple_spans_without_changing_other_text() {
        let source = "A=SECRET B=PASSWORD C=public";
        let report = ScanReport::new_with_candidates(
            vec![finding("a", 2, 8), finding("b", 11, 19)],
            Vec::new(),
        );

        assert_eq!(
            redact(source, &report).unwrap(),
            "A=[REDACTED] B=[REDACTED] C=public",
        );
    }

    #[test]
    fn duplicate_spans_emit_one_marker() {
        let report = ScanReport::new_with_candidates(
            vec![finding("one", 0, 6), finding("two", 0, 6)],
            Vec::new(),
        );

        assert_eq!(redact("SECRET", &report).unwrap(), "[REDACTED]");
    }

    #[test]
    fn overlapping_spans_are_merged_safely() {
        let source = "0123456789";
        let report = ScanReport::new_with_candidates(
            vec![finding("left", 2, 7), finding("right", 5, 9)],
            Vec::new(),
        );

        assert_eq!(redact(source, &report).unwrap(), "01[REDACTED]9");
    }

    #[test]
    fn adjacent_spans_remain_distinct() {
        let source = "ABCDEF";
        let report = ScanReport::new_with_candidates(
            vec![finding("left", 0, 3), finding("right", 3, 6)],
            Vec::new(),
        );

        assert_eq!(redact(source, &report).unwrap(), "[REDACTED][REDACTED]",);
    }

    #[test]
    fn secret_text_is_absent_from_output() {
        let source = "prefix SUPER_SECRET_VALUE suffix";
        let start = source.find("SUPER_SECRET_VALUE").unwrap();
        let end = start + "SUPER_SECRET_VALUE".len();
        let report =
            ScanReport::new_with_candidates(vec![finding("secret", start, end)], Vec::new());

        let output = redact(source, &report).unwrap();

        assert!(!output.contains("SUPER_SECRET_VALUE"));
        assert!(output.contains("[REDACTED]"));
    }
}
