//! Contextual validation for HTTP Basic authentication credentials.
//!
//! The source contains the encoded Basic credential. Validation decodes it
//! locally and requires an explicit `username:password`-like credential
//! structure without exposing or persisting the decoded value.

use base64::{Engine as _, engine::general_purpose::STANDARD};

use super::{
    context::ValidationContext,
    utils::{DEFAULT_KEY_WINDOW, key_matches_any, nearest_key},
};
use crate::validators::utils::is_obvious_placeholder;

const MAX_ENCODED_LEN: usize = 2048;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct HttpBasicValidation;

pub(crate) fn validate_http_basic(context: &ValidationContext<'_>) -> Option<HttpBasicValidation> {
    let candidate = context.candidate();

    if !(4..=MAX_ENCODED_LEN).contains(&candidate.len()) || !candidate.is_ascii() {
        return None;
    }

    let before = context.before_window(DEFAULT_KEY_WINDOW);
    let key = nearest_key(before)?;

    if !key_matches_any(key, &["authorization"])
        || !before.trim_end().to_ascii_lowercase().ends_with("basic")
    {
        return None;
    }

    let decoded = STANDARD.decode(candidate).ok()?;

    // Require canonical standard Base64. This rejects malformed padding,
    // truncated encodings and non-canonical representations.
    if STANDARD.encode(&decoded) != candidate {
        return None;
    }

    let separator = decoded.iter().position(|byte| *byte == b':')?;
    let username = &decoded[..separator];
    let password = &decoded[separator + 1..];

    if username.is_empty() || password.is_empty() {
        return None;
    }

    if username.iter().any(u8::is_ascii_control) || password.iter().any(u8::is_ascii_control) {
        return None;
    }

    let username_text = std::str::from_utf8(username).ok();
    let password_text = std::str::from_utf8(password).ok();

    if username_text.is_some_and(is_obvious_placeholder)
        || password_text.is_some_and(is_obvious_placeholder)
    {
        return None;
    }

    // Common documentation/example credentials. Keep this local to HTTP Basic
    // rather than broadening the global placeholder policy.
    if matches!(
        (username_text, password_text),
        (Some("user" | "username"), Some("password" | "passwd"))
    ) {
        return None;
    }

    // RFC-style documentation example.
    if decoded == b"Aladdin:open sesame" {
        return None;
    }

    Some(HttpBasicValidation)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context<'a>(source: &'a str, candidate: &'a str) -> ValidationContext<'a> {
        let start = source
            .find(candidate)
            .expect("fixture must contain candidate");
        ValidationContext::new(source, start..start + candidate.len())
    }

    #[test]
    fn accepts_valid_basic_credentials() {
        let encoded = STANDARD.encode("cribra:CorrectHorseBatteryStaple");
        let source = format!("Authorization: Basic {encoded}");

        assert!(validate_http_basic(&context(&source, &encoded)).is_some());
    }

    #[test]
    fn rejects_invalid_structure_and_documentation_examples() {
        for decoded in [
            "cribra",
            ":CorrectHorseBatteryStaple",
            "cribra:",
            "user:password",
            "username:password",
            "user:passwd",
            "Aladdin:open sesame",
        ] {
            let encoded = STANDARD.encode(decoded);
            let source = format!("Authorization: Basic {encoded}");

            assert!(
                validate_http_basic(&context(&source, &encoded)).is_none(),
                "unexpected acceptance for {decoded:?}"
            );
        }
    }

    #[test]
    fn rejects_non_authorization_context() {
        let encoded = STANDARD.encode("cribra:CorrectHorseBatteryStaple");
        let source = format!("value: Basic {encoded}");

        assert!(validate_http_basic(&context(&source, &encoded)).is_none());
    }
}
