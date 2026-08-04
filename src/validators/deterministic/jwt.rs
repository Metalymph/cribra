//! Structural validation for compact signed JWT/JWS values.
//!
//! This validator recognizes the three-segment JWS Compact Serialization used
//! by signed JWTs. It validates the textual envelope only; it does not decode
//! claims, select algorithms or verify cryptographic signatures.

use crate::validators::utils::{has_ascii_len, is_base64url_segment, is_obvious_placeholder};

const MIN_TOKEN_LEN: usize = 32;
const MAX_TOKEN_LEN: usize = 16 * 1024;

/// Compact token family recognized by the validator.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum JwtKind {
    /// Three-segment signed JWT/JWS compact serialization.
    SignedCompactJws,
}

/// Successful JWT/JWS structural validation.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct JwtValidation {
    kind: JwtKind,
}

impl JwtValidation {
    pub(crate) const fn kind(self) -> JwtKind {
        self.kind
    }
}

/// Validates the structural envelope of a signed compact JWT/JWS.
///
/// A successful result means only that the candidate has exactly three
/// non-empty Base64URL segments and a header segment that commonly represents
/// a JSON object (`eyJ...`). Signature validity is not checked.
pub(crate) fn validate_jwt(candidate: &str) -> Option<JwtValidation> {
    if !has_ascii_len(candidate, MIN_TOKEN_LEN, MAX_TOKEN_LEN) || is_obvious_placeholder(candidate)
    {
        return None;
    }

    let mut segments = candidate.split('.');

    let header = segments.next()?;
    let payload = segments.next()?;
    let signature = segments.next()?;

    if segments.next().is_some()
        || !header.starts_with("eyJ")
        || !is_base64url_segment(header)
        || !is_base64url_segment(payload)
        || !is_base64url_segment(signature)
    {
        return None;
    }

    Some(JwtValidation {
        kind: JwtKind::SignedCompactJws,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const JWT: &str = concat!(
        "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.",
        "eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIn0.",
        "SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
    );

    #[test]
    fn accepts_three_segment_compact_jws() {
        assert_eq!(
            validate_jwt(JWT).map(JwtValidation::kind),
            Some(JwtKind::SignedCompactJws),
        );
    }

    #[test]
    fn rejects_two_or_four_segments() {
        assert!(validate_jwt("eyJheader.payload").is_none());
        assert!(validate_jwt("eyJheader.payload.signature.extra").is_none());
    }

    #[test]
    fn rejects_invalid_base64url_characters() {
        let invalid = concat!(
            "eyJhbGciOiJIUzI1NiJ9.",
            "eyJzdWIiOiIxMjM0NTY3ODkwIn0.",
            "signature+invalid",
        );

        assert!(validate_jwt(invalid).is_none());
    }

    #[test]
    fn rejects_non_json_like_header_prefix() {
        let invalid = concat!(
            "YWJjZGVmZ2hpamtsbW5vcA.",
            "eyJzdWIiOiIxMjM0NTY3ODkwIn0.",
            "SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
        );

        assert!(validate_jwt(invalid).is_none());
    }

    #[test]
    fn rejects_empty_signature() {
        assert!(validate_jwt("eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.").is_none());
    }
}
