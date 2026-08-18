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
    validators::utils::is_obvious_placeholder,
};

const GROUP_LEN: usize = 4;
const GROUP_COUNT: usize = 4;
const SEPARATOR_COUNT: usize = GROUP_COUNT - 1;
const RECOVERY_LIKE_LEN: usize = GROUP_LEN * GROUP_COUNT + SEPARATOR_COUNT;

/// Detects structurally plausible sensitive values without promoting them to findings.
///
/// The scanner invokes this path independently from compiled rule matching.
/// Candidates that overlap an accepted finding are discarded before the
/// immutable report is materialized, so one span is never simultaneously
/// presented as both a confirmed finding and an ambiguous candidate.
pub(crate) fn detect_sensitive_candidates(source: &str) -> Vec<SensitiveCandidate> {
    let bytes = source.as_bytes();
    let mut candidates = Vec::new();
    let mut start = 0;

    while start + RECOVERY_LIKE_LEN <= bytes.len() {
        if is_recovery_like_at(bytes, start) {
            let end = start + RECOVERY_LIKE_LEN;
            let mut location = Location::from_span(start, end);
            let (line, column) = source_position(source, start);
            location.set_position(line, column);

            candidates.push(SensitiveCandidate::new(
                SensitiveCandidateKind::RecoveryLikeCode,
                location,
                CandidateEvidence::Structural,
            ));

            // A valid token cannot start inside the span we just accepted.
            start = end;
        } else {
            start += 1;
        }
    }

    candidates
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

fn source_position(source: &str, target: usize) -> (usize, usize) {
    debug_assert!(source.is_char_boundary(target));

    let mut line = 1;
    let mut column = 1;

    for character in source[..target].chars() {
        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }

    (line, column)
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
