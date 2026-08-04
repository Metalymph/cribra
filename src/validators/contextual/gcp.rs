//! Contextual validation for Google Cloud service-account credentials.
//!
//! Service-account private material is identified through the canonical JSON
//! field names and PEM structure rather than through a standalone opaque-token
//! prefix.

use super::{
    context::ValidationContext,
    utils::{DEFAULT_KEY_WINDOW, contains_ascii_case_insensitive, key_matches_any, nearest_key},
};
use crate::validators::utils::is_obvious_placeholder;

/// Google Cloud credential artifact recognized by contextual validation.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum GcpCredentialKind {
    /// Service-account private key under the `private_key` field.
    ServiceAccountPrivateKey,

    /// Private-key identifier under the `private_key_id` field.
    ServiceAccountPrivateKeyId,

    /// Client secret under the `client_secret` field.
    OAuthClientSecret,
}

/// Successful Google Cloud contextual validation.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct GcpValidation {
    kind: GcpCredentialKind,
}

impl GcpValidation {
    pub(crate) const fn kind(self) -> GcpCredentialKind {
        self.kind
    }
}

/// Validates a candidate found in Google Cloud credential context.
pub(crate) fn validate_gcp(context: &ValidationContext<'_>) -> Option<GcpValidation> {
    let candidate = context.candidate();

    if candidate.is_empty() || is_obvious_placeholder(candidate) {
        return None;
    }

    let before = context.before_window(DEFAULT_KEY_WINDOW);
    let key = nearest_key(before)?;

    if key_matches_any(key, &["private_key"]) && looks_like_private_key(candidate) {
        return Some(GcpValidation {
            kind: GcpCredentialKind::ServiceAccountPrivateKey,
        });
    }

    if key_matches_any(key, &["private_key_id"])
        && candidate.len() >= 16
        && candidate.len() <= 128
        && candidate.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Some(GcpValidation {
            kind: GcpCredentialKind::ServiceAccountPrivateKeyId,
        });
    }

    if key_matches_any(key, &["client_secret"])
        && candidate.len() >= 16
        && candidate.len() <= 255
        && candidate.is_ascii()
    {
        return Some(GcpValidation {
            kind: GcpCredentialKind::OAuthClientSecret,
        });
    }

    None
}

fn looks_like_private_key(candidate: &str) -> bool {
    candidate.len() >= 64
        && contains_ascii_case_insensitive(candidate, "begin private key")
        && contains_ascii_case_insensitive(candidate, "end private key")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context<'a>(source: &'a str, value: &'a str) -> ValidationContext<'a> {
        let start = source.find(value).expect("fixture must contain value");
        ValidationContext::new(source, start..start + value.len())
    }

    #[test]
    fn recognizes_service_account_private_key_id() {
        let value = "0123456789abcdef0123456789abcdef01234567";
        let source = format!(r#""private_key_id": "{value}""#);

        assert_eq!(
            validate_gcp(&context(&source, value)).map(GcpValidation::kind),
            Some(GcpCredentialKind::ServiceAccountPrivateKeyId),
        );
    }

    #[test]
    fn recognizes_service_account_private_key() {
        let value = concat!(
            "-----BEGIN PRIVATE KEY-----\n",
            "MIIEvQIBADANBgkqhkiG9w0BAQEFAASC",
            "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ\n",
            "-----END PRIVATE KEY-----",
        );
        let source = format!(r#""private_key": "{value}""#);

        assert_eq!(
            validate_gcp(&context(&source, value)).map(GcpValidation::kind),
            Some(GcpCredentialKind::ServiceAccountPrivateKey),
        );
    }

    #[test]
    fn rejects_private_key_id_without_field_context() {
        let value = "0123456789abcdef0123456789abcdef01234567";
        assert!(validate_gcp(&context(value, value)).is_none());
    }
}
