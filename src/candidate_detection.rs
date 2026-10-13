//! Internal structural detection for ambiguous sensitive values.
//!
//! This module deliberately does not create [`Finding`](crate::Finding) values.
//! It recognizes only narrow, review-worthy structures for which the scanner
//! lacks enough semantic evidence to claim an actual secret detection.
//!
//! The first supported family is a grouped recovery-like code:
//!
//! ```text
//! ABCD-EFGH-IJKL-MNOP
//! ```
//!
//! The shape is intentionally conservative:
//!
//! - exactly four groups of four uppercase ASCII alphanumeric characters;
//! - `-` separators between groups;
//! - token boundaries on both sides;
//! - numeric-only values are rejected;
//! - hexadecimal-only values are rejected;
//! - obvious placeholder/repeated values are rejected.
//!
//! These constraints reduce noise from ordinary numbers, UUID/hash fragments,
//! and documentation placeholders while still surfacing ambiguous values that
//! merit manual review.

use crate::{
    CandidateEvidence, Location, SensitiveCandidate, SensitiveCandidateKind,
    compiled_rule::{IncrementalWork, WorkMetric},
    validators::utils::is_obvious_placeholder,
};

const GROUP_LEN: usize = 4;
const GROUP_COUNT: usize = 4;
const SEPARATOR_COUNT: usize = GROUP_COUNT - 1;
pub(crate) const RECOVERY_LIKE_LEN: usize = GROUP_LEN * GROUP_COUNT + SEPARATOR_COUNT;

/// Detects structurally plausible sensitive values without promoting them to findings.
///
/// The scanner invokes this path independently from compiled rule matching.
/// Candidates that overlap an accepted finding are discarded before the
/// immutable report is materialized, so one span is never simultaneously
/// presented as both a confirmed finding and an ambiguous candidate.
pub(crate) fn detect_sensitive_candidates(source: &str) -> Vec<SensitiveCandidate> {
    let spans = detect_owned_spans(source, 0..source.len(), None)
        .expect("unmetered structural detection cannot overflow work counters");
    materialize_candidates(source, spans)
}

/// Enumerate owned starts using the same predicate and separator traversal as
/// whole-source detection. Boundaries are inspected in the complete haystack.
pub(crate) fn detect_owned_spans(
    source: &str,
    starts: std::ops::Range<usize>,
    work: Option<&mut IncrementalWork>,
) -> Option<Vec<(usize, usize)>> {
    let mut spans = Vec::new();
    detect_owned_spans_with_emit(source, starts, work, &mut |start, end| {
        spans.push((start, end));
        Ok(())
    })
    .ok()?;
    Some(spans)
}

pub(crate) fn detect_owned_spans_with_emit(
    source: &str,
    starts: std::ops::Range<usize>,
    mut work: Option<&mut IncrementalWork>,
    emit: &mut impl FnMut(usize, usize) -> Result<(), crate::IncrementalError>,
) -> Result<(), crate::IncrementalError> {
    let bytes = source.as_bytes();
    if starts.is_empty() {
        return Ok(());
    }
    if let Some(work) = work.as_deref_mut() {
        let left = starts.start.saturating_sub(1);
        let right = starts
            .end
            .checked_add(RECOVERY_LIKE_LEN)
            .unwrap_or(bytes.len())
            .min(bytes.len());
        work.record(
            WorkMetric::StructuralBytes,
            right
                .checked_sub(left)
                .ok_or(crate::IncrementalError::ExecutionInvariantViolated)?,
        )
        .ok_or(crate::IncrementalError::ExecutionInvariantViolated)?;
    }
    // The first separator is four bytes after an owned start. A valid token
    // cannot contain another valid start (its preceding byte is token-like).
    let Some(mut separator) = starts.start.checked_add(GROUP_LEN) else {
        return Ok(());
    };
    while separator < bytes.len() {
        let start = separator - GROUP_LEN;
        if start >= starts.end {
            break;
        }
        if bytes[separator] != b'-' {
            separator += 1;
            continue;
        }
        if let Some(end) = start.checked_add(RECOVERY_LIKE_LEN)
            && end <= bytes.len()
        {
            if let Some(work) = work.as_deref_mut() {
                work.record(WorkMetric::StructuralPredicates, 1)
                    .ok_or(crate::IncrementalError::ExecutionInvariantViolated)?;
            }
            if is_recovery_like_at(bytes, start) {
                emit(start, end)?;
                separator = end.checked_add(GROUP_LEN).unwrap_or(bytes.len());
                continue;
            }
        }
        separator += 1;
    }
    Ok(())
}

pub(crate) fn materialize_candidates(
    source: &str,
    spans: Vec<(usize, usize)>,
) -> Vec<SensitiveCandidate> {
    let mut candidates = Vec::with_capacity(spans.len());
    materialize_candidates_with_emit(source, spans, &mut |candidate| {
        candidates.push(candidate);
        Ok(())
    })
    .expect("Vec emission cannot fail");
    candidates
}

pub(crate) fn materialize_candidates_with_emit(
    source: &str,
    spans: impl IntoIterator<Item = (usize, usize)>,
    emit: &mut impl FnMut(SensitiveCandidate) -> Result<(), crate::IncrementalError>,
) -> Result<(), crate::IncrementalError> {
    let mut cursor = 0;
    let mut line = 1;
    let mut column = 1;

    for (start, end) in spans {
        advance_position(source, &mut cursor, start, &mut line, &mut column);

        let mut location = Location::from_span(start, end);
        location.set_position(line, column);
        emit(SensitiveCandidate::new(
            SensitiveCandidateKind::RecoveryLikeCode,
            location,
            CandidateEvidence::Structural,
        ))?;
    }

    Ok(())
}

fn is_recovery_like_at(bytes: &[u8], start: usize) -> bool {
    let end = start + RECOVERY_LIKE_LEN;

    if !has_token_boundaries(bytes, start, end) {
        return false;
    }

    let token = &bytes[start..end];

    if !has_grouped_shape(token) {
        return false;
    }

    let mut compact = [0_u8; GROUP_LEN * GROUP_COUNT];
    let mut compact_index = 0;

    for &byte in token {
        if byte != b'-' {
            compact[compact_index] = byte;
            compact_index += 1;
        }
    }

    if compact.iter().all(u8::is_ascii_digit) {
        return false;
    }

    if compact.iter().all(u8::is_ascii_hexdigit) {
        return false;
    }

    let compact_str =
        core::str::from_utf8(&compact).expect("validated recovery-like bytes are ASCII");
    !is_obvious_placeholder(compact_str)
}

fn has_grouped_shape(token: &[u8]) -> bool {
    debug_assert_eq!(token.len(), RECOVERY_LIKE_LEN);

    for (index, &byte) in token.iter().enumerate() {
        let separator = matches!(index, 4 | 9 | 14);

        if separator {
            if byte != b'-' {
                return false;
            }
        } else if !(byte.is_ascii_uppercase() || byte.is_ascii_digit()) {
            return false;
        }
    }

    true
}

fn has_token_boundaries(bytes: &[u8], start: usize, end: usize) -> bool {
    let boundary_byte = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'-';

    let before_is_clear = start == 0 || !boundary_byte(bytes[start - 1]);
    let after_is_clear = end == bytes.len() || !boundary_byte(bytes[end]);

    before_is_clear && after_is_clear
}

fn advance_position(
    source: &str,
    cursor: &mut usize,
    target: usize,
    line: &mut usize,
    column: &mut usize,
) {
    debug_assert!(*cursor <= target);
    debug_assert!(source.is_char_boundary(*cursor));
    debug_assert!(source.is_char_boundary(target));

    for character in source[*cursor..target].chars() {
        if character == '\n' {
            *line += 1;
            *column = 1;
        } else {
            *column += 1;
        }
    }

    *cursor = target;
}

/// Fixed window: left boundary, 19 candidate bytes, right boundary. The
/// initial and EOF sentinels represent genuine source boundaries only.
/// Each byte carries its scalar-start coordinate; accepted tokens are ASCII.
#[derive(Debug)]
pub(crate) struct StreamingCandidateDetector {
    bytes: [u8; RECOVERY_LIKE_LEN + 2],
    positions: [crate::source_coordinates::SourceCoordinates; RECOVERY_LIKE_LEN + 2],
    len: usize,
    cursor: crate::source_coordinates::SourceCoordinates,
}

impl Default for StreamingCandidateDetector {
    fn default() -> Self {
        Self {
            bytes: [0; RECOVERY_LIKE_LEN + 2],
            positions: [crate::source_coordinates::SourceCoordinates::default();
                RECOVERY_LIKE_LEN + 2],
            len: 1,
            cursor: crate::source_coordinates::SourceCoordinates::default(),
        }
    }
}

impl StreamingCandidateDetector {
    fn byte(&mut self, byte: u8) -> Option<SensitiveCandidate> {
        if self.len == self.bytes.len() {
            self.bytes.copy_within(1.., 0);
            self.positions.copy_within(1.., 0);
            self.len -= 1;
        }
        self.bytes[self.len] = byte;
        self.positions[self.len] = self.cursor;
        self.len += 1;
        if self.len != self.bytes.len() || !is_recovery_like_at(&self.bytes, 1) {
            return None;
        }
        let start = self.positions[1];
        // The 19 ASCII bytes have already been consumed. The addition is
        // representable because the checked cursor reached their end.
        let mut location =
            Location::from_span(start.byte_offset(), start.byte_offset() + RECOVERY_LIKE_LEN);
        location.set_position(start.line(), start.column());
        Some(SensitiveCandidate::new(
            SensitiveCandidateKind::RecoveryLikeCode,
            location,
            CandidateEvidence::Structural,
        ))
    }

    pub(crate) fn push(
        &mut self,
        text: &str,
        base: usize,
        emit: &mut impl FnMut(
            SensitiveCandidate,
        ) -> Result<(), crate::source_session::SourceSessionError>,
    ) -> Result<(), crate::source_session::SourceSessionError> {
        if self.cursor.byte_offset() != base {
            return Err(crate::source_session::SourceSessionError::CoordinateInvariantViolation);
        }
        for character in text.chars() {
            let mut encoded = [0; 4];
            let scalar = character.encode_utf8(&mut encoded);
            let mut next = self.cursor;
            next.checked_advance(scalar)?;
            for &byte in scalar.as_bytes() {
                if let Some(candidate) = self.byte(byte) {
                    emit(candidate)?;
                }
            }
            self.cursor = next;
        }
        Ok(())
    }

    pub(crate) fn finish(&mut self) -> Option<SensitiveCandidate> {
        self.byte(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans(source: &str) -> Vec<&str> {
        detect_sensitive_candidates(source)
            .into_iter()
            .map(|candidate| {
                let range = candidate.location().byte_range();
                &source[range]
            })
            .collect()
    }

    #[test]
    fn detects_isolated_grouped_recovery_like_code() {
        let source = "ABCD-EFGH-IJKL-MNOP";
        let candidates = detect_sensitive_candidates(source);

        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].kind(),
            SensitiveCandidateKind::RecoveryLikeCode
        );
        assert_eq!(candidates[0].evidence(), CandidateEvidence::Structural);
        assert_eq!(candidates[0].location().byte_range(), 0..19);
        assert_eq!(candidates[0].location().line(), 1);
        assert_eq!(candidates[0].location().column(), 1);
    }

    #[test]
    fn detects_candidate_inside_multiline_unicode_source() {
        let source = "header 😀\nvalue: ABCD-EFGH-IJKL-MNOP\nfooter";
        let candidates = detect_sensitive_candidates(source);

        assert_eq!(candidates.len(), 1);
        assert_eq!(
            &source[candidates[0].location().byte_range()],
            "ABCD-EFGH-IJKL-MNOP"
        );
        assert_eq!(candidates[0].location().line(), 2);
        assert_eq!(candidates[0].location().column(), 8);
    }

    #[test]
    fn detects_multiple_candidates_in_source_order() {
        let source = "ABCD-EFGH-IJKL-MNOP\nQRST-UVWX-YZ12-3456";
        assert_eq!(
            spans(source),
            ["ABCD-EFGH-IJKL-MNOP", "QRST-UVWX-YZ12-3456"]
        );
    }

    #[test]
    fn rejects_numeric_only_grouped_values() {
        assert!(detect_sensitive_candidates("1234-5678-9012-3456").is_empty());
    }

    #[test]
    fn rejects_hexadecimal_only_grouped_values() {
        assert!(detect_sensitive_candidates("ABCD-EF12-3456-7890").is_empty());
    }

    #[test]
    fn rejects_lowercase_and_mixed_case_shapes() {
        assert!(detect_sensitive_candidates("abcd-EFGH-IJKL-MNOP").is_empty());
        assert!(detect_sensitive_candidates("AbCD-EFGH-IJKL-MNOP").is_empty());
    }

    #[test]
    fn rejects_obvious_placeholder_values() {
        assert!(detect_sensitive_candidates("XXXX-XXXX-XXXX-XXXX").is_empty());
    }

    #[test]
    fn rejects_partial_or_longer_tokens() {
        assert!(detect_sensitive_candidates("ABCD-EFGH-IJKL").is_empty());
        assert!(detect_sensitive_candidates("XABCD-EFGH-IJKL-MNOP").is_empty());
        assert!(detect_sensitive_candidates("ABCD-EFGH-IJKL-MNOPQ").is_empty());
        assert!(detect_sensitive_candidates("ABCD-EFGH-IJKL-MNOP-QRST").is_empty());
    }

    #[test]
    fn accepts_safe_surrounding_punctuation() {
        let source = "(ABCD-EFGH-IJKL-MNOP),";
        let candidates = detect_sensitive_candidates(source);

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].location().byte_range(), 1..20);
    }

    #[test]
    fn preserves_positions_for_multiple_candidates_after_unicode_and_newlines() {
        let source = "😀 header\nABCD-EFGH-IJKL-MNOP\nαβγ QRST-UVWX-YZ12-3456\n";

        let candidates = detect_sensitive_candidates(source);

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].location().line(), 2);
        assert_eq!(candidates[0].location().column(), 1);
        assert_eq!(candidates[1].location().line(), 3);
        assert_eq!(candidates[1].location().column(), 5);
    }

    #[test]
    fn hyphen_heavy_non_candidates_remain_rejected() {
        let source = "ordinary-value-with-many-hyphens---still-not-a-recovery-code\n".repeat(256);

        assert!(detect_sensitive_candidates(&source).is_empty());
    }

    #[test]
    fn candidate_model_remains_distinct_from_finding_semantics() {
        let candidate = detect_sensitive_candidates("ABCD-EFGH-IJKL-MNOP")
            .into_iter()
            .next()
            .expect("candidate should be detected");

        assert_eq!(candidate.evidence(), CandidateEvidence::Structural);

        // The public candidate type intentionally exposes no severity,
        // finding confidence, remediation, rule identifier or matched value.
        let _ = candidate.kind();
        let _ = candidate.location();
    }
}
