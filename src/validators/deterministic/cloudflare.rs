//! Structural validation for Cloudflare scannable API credentials.
//!
//! Cloudflare's current scannable credential formats use a type-specific
//! prefix, 40 alphanumeric characters and an 8-character hexadecimal checksum.
//! This validator checks the complete public shape. Checksum verification can
//! be added later when the exact checksum construction is part of the stable
//! validator contract.

use crate::validators::utils::{is_obvious_placeholder, non_empty_ascii_with};

const BODY_LEN: usize = 40;
const CHECKSUM_LEN: usize = 8;

/// Cloudflare credential family recognized by the validator.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum CloudflareTokenKind {
    /// Global API key beginning with `cfk_`.
    GlobalApiKey,

    /// User-scoped API token beginning with `cfut_`.
    UserApiToken,

    /// Account-owned API token beginning with `cfat_`.
    AccountApiToken,
}

/// Successful Cloudflare token structural validation.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct CloudflareValidation {
    kind: CloudflareTokenKind,
}

impl CloudflareValidation {
    pub(crate) const fn kind(self) -> CloudflareTokenKind {
        self.kind
    }
}

/// Validates a current prefixed Cloudflare API credential.
pub(crate) fn validate_cloudflare_token(candidate: &str) -> Option<CloudflareValidation> {
    if !candidate.is_ascii() || is_obvious_placeholder(candidate) {
        return None;
    }

    let (payload, kind) = if let Some(payload) = candidate.strip_prefix("cfk_") {
        (payload, CloudflareTokenKind::GlobalApiKey)
    } else if let Some(payload) = candidate.strip_prefix("cfut_") {
        (payload, CloudflareTokenKind::UserApiToken)
    } else {
        let payload = candidate.strip_prefix("cfat_")?;
        (payload, CloudflareTokenKind::AccountApiToken)
    };

    if payload.len() != BODY_LEN + CHECKSUM_LEN {
        return None;
    }

    let (body, checksum) = payload.split_at(BODY_LEN);

    if !non_empty_ascii_with(body, |byte| byte.is_ascii_alphanumeric())
        || !non_empty_ascii_with(checksum, |byte| byte.is_ascii_hexdigit())
    {
        return None;
    }

    Some(CloudflareValidation { kind })
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &str = "AbCdEf0123456789AbCdEf0123456789AbCdEf01";
    const CHECKSUM: &str = "a1B2c3D4";

    #[test]
    fn accepts_current_scannable_formats() {
        let cases = [
            (
                format!("cfk_{BODY}{CHECKSUM}"),
                CloudflareTokenKind::GlobalApiKey,
            ),
            (
                format!("cfut_{BODY}{CHECKSUM}"),
                CloudflareTokenKind::UserApiToken,
            ),
            (
                format!("cfat_{BODY}{CHECKSUM}"),
                CloudflareTokenKind::AccountApiToken,
            ),
        ];

        for (candidate, expected) in cases {
            assert_eq!(
                validate_cloudflare_token(&candidate).map(CloudflareValidation::kind),
                Some(expected),
            );
        }
    }

    #[test]
    fn rejects_wrong_body_or_checksum_length() {
        assert!(validate_cloudflare_token("cfut_short").is_none());
        assert!(validate_cloudflare_token(&format!("cfut_{BODY}abc")).is_none());
    }

    #[test]
    fn rejects_non_hex_checksum() {
        assert!(validate_cloudflare_token(&format!("cfut_{BODY}ZZZZZZZZ")).is_none());
    }

    #[test]
    fn rejects_legacy_unprefixed_tokens() {
        assert!(validate_cloudflare_token(BODY).is_none());
    }
}
