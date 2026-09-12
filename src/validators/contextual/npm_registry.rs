//! Contextual validation for registry-scoped npm credentials.
//!
//! npm authentication material is accepted only through dedicated `.npmrc`
//! registry-scoped rules. Decoded material is used transiently for structural
//! validation and is never exposed as the finding span.

use base64::{Engine as _, engine::general_purpose::STANDARD};

use super::{
    context::ValidationContext,
    utils::{DEFAULT_KEY_WINDOW, key_matches_any, nearest_key},
};
use crate::validators::utils::is_obvious_placeholder;

const MIN_TOKEN_LEN: usize = 8;
const MAX_TOKEN_LEN: usize = 2048;
const MAX_ENCODED_LEN: usize = 4096;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum NpmRegistryCredentialKind {
    AuthToken,
    Auth,
    Password,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct NpmRegistryValidation {
    kind: NpmRegistryCredentialKind,
}

impl NpmRegistryValidation {
    pub(crate) const fn kind(self) -> NpmRegistryCredentialKind {
        self.kind
    }
}

pub(crate) fn validate_npm_registry(
    context: &ValidationContext<'_>,
) -> Option<NpmRegistryValidation> {
    let candidate = context.candidate();

    if candidate.is_empty() || candidate.chars().any(char::is_control) {
        return None;
    }

    let key = nearest_key(context.before_window(DEFAULT_KEY_WINDOW))?;

    if key_matches_any(key, &["_authToken"]) {
        return validate_auth_token(candidate);
    }

    if key_matches_any(key, &["_auth"]) {
        return validate_auth(candidate);
    }

    if key_matches_any(key, &["_password"]) {
        return validate_password(candidate);
    }

    None
}

fn validate_auth_token(candidate: &str) -> Option<NpmRegistryValidation> {
    if !(MIN_TOKEN_LEN..=MAX_TOKEN_LEN).contains(&candidate.len())
        || is_obvious_placeholder(candidate)
        || is_documentation_value(candidate)
    {
        return None;
    }

    Some(NpmRegistryValidation {
        kind: NpmRegistryCredentialKind::AuthToken,
    })
}

fn validate_auth(candidate: &str) -> Option<NpmRegistryValidation> {
    let decoded = decode_canonical(candidate)?;

    let separator = decoded.iter().position(|byte| *byte == b':')?;
    let username = &decoded[..separator];
    let password = &decoded[separator + 1..];

    if username.is_empty() || password.is_empty() {
        return None;
    }

    let username = std::str::from_utf8(username).ok()?;
    let password = std::str::from_utf8(password).ok()?;

    if username.chars().any(char::is_control)
        || password.chars().any(char::is_control)
        || is_obvious_placeholder(username)
        || is_obvious_placeholder(password)
        || is_documentation_pair(username, password)
    {
        return None;
    }

    Some(NpmRegistryValidation {
        kind: NpmRegistryCredentialKind::Auth,
    })
}

fn validate_password(candidate: &str) -> Option<NpmRegistryValidation> {
    let decoded = decode_canonical(candidate)?;
    let password = std::str::from_utf8(&decoded).ok()?;

    if password.is_empty()
        || password.chars().any(char::is_control)
        || is_obvious_placeholder(password)
        || is_documentation_value(password)
    {
        return None;
    }

    Some(NpmRegistryValidation {
        kind: NpmRegistryCredentialKind::Password,
    })
}

fn decode_canonical(candidate: &str) -> Option<Vec<u8>> {
    if !(4..=MAX_ENCODED_LEN).contains(&candidate.len()) || !candidate.is_ascii() {
        return None;
    }

    let decoded = STANDARD.decode(candidate).ok()?;

    (STANDARD.encode(&decoded) == candidate).then_some(decoded)
}

fn is_documentation_value(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "password"
            | "passwd"
            | "npm_token"
            | "your_token"
            | "your_token_here"
            | "your_auth_token"
            | "example_token"
            | "replace_me"
    )
}

fn is_documentation_pair(username: &str, password: &str) -> bool {
    matches!(
        (
            username.to_ascii_lowercase().as_str(),
            password.to_ascii_lowercase().as_str()
        ),
        ("user" | "username", "password" | "passwd")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context<'a>(source: &'a str, value: &'a str) -> ValidationContext<'a> {
        let start = source.find(value).expect("fixture must contain value");
        ValidationContext::new(source, start..start + value.len())
    }

    #[test]
    fn recognizes_registry_scoped_auth_token() {
        let token = "npm_AbCdEfGhIjKlMnOpQrStUvWxYz012345";
        let source = format!("//registry.npmjs.org/:_authToken={token}");

        assert_eq!(
            validate_npm_registry(&context(&source, token)).map(NpmRegistryValidation::kind),
            Some(NpmRegistryCredentialKind::AuthToken),
        );
    }

    #[test]
    fn recognizes_registry_scoped_auth() {
        let encoded = STANDARD.encode("cribra:CorrectHorseBatteryStaple");
        let source = format!("//registry.example.com/:_auth={encoded}");

        assert_eq!(
            validate_npm_registry(&context(&source, &encoded)).map(NpmRegistryValidation::kind),
            Some(NpmRegistryCredentialKind::Auth),
        );
    }

    #[test]
    fn recognizes_registry_scoped_password() {
        let encoded = STANDARD.encode("CorrectHorseBatteryStaple");
        let source = format!("//registry.example.com/:_password={encoded}");

        assert_eq!(
            validate_npm_registry(&context(&source, &encoded)).map(NpmRegistryValidation::kind),
            Some(NpmRegistryCredentialKind::Password),
        );
    }

    #[test]
    fn rejects_placeholders_and_malformed_encoded_credentials() {
        for (key, value) in [
            ("_authToken", "your_token_here"),
            ("_auth", "not-base64"),
            ("_password", "not-base64"),
        ] {
            let source = format!("//registry.example.com/:{key}={value}");

            assert!(
                validate_npm_registry(&context(&source, value)).is_none(),
                "unexpected npm credential validation for {source:?}",
            );
        }
    }

    #[test]
    fn rejects_documentation_auth_pair() {
        let encoded = STANDARD.encode("user:password");
        let source = format!("//registry.example.com/:_auth={encoded}");

        assert!(validate_npm_registry(&context(&source, &encoded)).is_none());
    }
}
