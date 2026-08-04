//! Shared helpers for provider-specific candidate validators.
//!
//! These helpers operate on borrowed text and allocate nothing. They are kept
//! private to the crate so validator internals can evolve without expanding the
//! public API.

/// Returns `true` when `value` is non-empty ASCII text whose bytes all satisfy
/// `predicate`.
#[inline]
pub(crate) fn non_empty_ascii_with(value: &str, predicate: impl Fn(u8) -> bool) -> bool {
    !value.is_empty() && value.is_ascii() && value.bytes().all(predicate)
}

/// Returns `true` for the common opaque-token alphabet:
/// ASCII alphanumeric characters plus `_`.
#[inline]
pub(crate) const fn is_opaque_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Returns `true` for URL-safe Base64 characters without padding.
#[inline]
pub(crate) const fn is_base64url_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')
}

/// Returns `true` when `segment` is a non-empty unpadded Base64URL segment.
#[inline]
pub(crate) fn is_base64url_segment(segment: &str) -> bool {
    non_empty_ascii_with(segment, is_base64url_byte)
}

/// Returns `true` when `value` is a non-empty ASCII decimal integer.
#[inline]
pub(crate) fn is_ascii_decimal(value: &str) -> bool {
    non_empty_ascii_with(value, |byte| byte.is_ascii_digit())
}

/// Returns `true` when `value` is ASCII and its byte length is inside the
/// inclusive range.
#[inline]
pub(crate) fn has_ascii_len(value: &str, minimum: usize, maximum: usize) -> bool {
    value.is_ascii() && (minimum..=maximum).contains(&value.len())
}

/// Rejects common documentation, example and placeholder values.
///
/// This is deliberately conservative. Provider validators may add stricter
/// provider-specific placeholder checks.
pub(crate) fn is_obvious_placeholder(value: &str) -> bool {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return true;
    }

    let normalized = trimmed.to_ascii_lowercase();

    const EXACT_PLACEHOLDERS: &[&str] = &[
        "changeme",
        "change_me",
        "example",
        "example_token",
        "placeholder",
        "redacted",
        "secret",
        "token",
        "your_api_key",
        "your_api_key_here",
        "your_token",
        "your_token_here",
        "[redacted]",
        "<redacted>",
        "***",
    ];

    const EMBEDDED_MARKERS: &[&str] = &[
        "your_api_key_here",
        "your_token_here",
        "placeholder",
        "redacted",
        "changeme",
    ];

    EXACT_PLACEHOLDERS.contains(&normalized.as_str())
        || EMBEDDED_MARKERS
            .iter()
            .any(|marker| normalized.contains(marker))
        || has_single_repeated_ascii_byte(trimmed)
}

/// Returns `true` when all bytes are the same ASCII byte and the value contains
/// at least four bytes.
fn has_single_repeated_ascii_byte(value: &str) -> bool {
    if value.len() < 4 || !value.is_ascii() {
        return false;
    }

    let first = value.as_bytes()[0];
    value.as_bytes()[1..].iter().all(|byte| *byte == first)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_base64url_segments() {
        assert!(is_base64url_segment("abc_DEF-123"));
        assert!(!is_base64url_segment(""));
        assert!(!is_base64url_segment("abc+def"));
        assert!(!is_base64url_segment("abc="));
    }

    #[test]
    fn validates_ascii_decimal_values() {
        assert!(is_ascii_decimal("123456"));
        assert!(!is_ascii_decimal(""));
        assert!(!is_ascii_decimal("12a"));
    }

    #[test]
    fn rejects_obvious_placeholders() {
        assert!(is_obvious_placeholder("your_api_key_here"));
        assert!(is_obvious_placeholder("xxxxxxxx"));
        assert!(is_obvious_placeholder("[REDACTED]"));
        assert!(!is_obvious_placeholder("aB3_dE7_kL9"));
    }
}
