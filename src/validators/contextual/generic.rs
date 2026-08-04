//! Contextual validation for generic secret-bearing fields.
//!
//! This validator is intentionally conservative and is used only when a field
//! explicitly identifies a token, API key or secret value.

use super::{
    context::ValidationContext,
    utils::{DEFAULT_KEY_WINDOW, key_matches_any, nearest_key},
};
use crate::validators::utils::is_obvious_placeholder;

/// Generic credential field recognized by contextual validation.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum GenericCredentialKind {
    ApiKey,
    Token,
    Secret,
}

/// Successful generic credential contextual validation.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct GenericValidation {
    kind: GenericCredentialKind,
}

impl GenericValidation {
    pub(crate) const fn kind(self) -> GenericCredentialKind {
        self.kind
    }
}

/// Validates an opaque value assigned to an explicit generic credential field.
pub(crate) fn validate_generic_credential(
    context: &ValidationContext<'_>,
) -> Option<GenericValidation> {
    let candidate = context.candidate();

    if !(16..=2048).contains(&candidate.len())
        || candidate.chars().any(char::is_control)
        || is_obvious_placeholder(candidate)
    {
        return None;
    }

    let key = nearest_key(context.before_window(DEFAULT_KEY_WINDOW))?;

    let kind = if key_matches_any(key, &["api_key", "apikey", "api_token", "access_key"]) {
        GenericCredentialKind::ApiKey
    } else if key_matches_any(
        key,
        &["token", "access_token", "auth_token", "bearer_token"],
    ) {
        GenericCredentialKind::Token
    } else if key_matches_any(
        key,
        &["secret", "secret_key", "signing_secret", "webhook_secret"],
    ) {
        GenericCredentialKind::Secret
    } else {
        return None;
    };

    Some(GenericValidation { kind })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context<'a>(source: &'a str, value: &'a str) -> ValidationContext<'a> {
        let start = source.find(value).expect("fixture must contain value");
        ValidationContext::new(source, start..start + value.len())
    }

    #[test]
    fn recognizes_explicit_api_key_field() {
        let value = "AbCdEfGhIjKlMnOpQrStUvWx";
        let source = format!("API_KEY={value}");

        assert_eq!(
            validate_generic_credential(&context(&source, value)).map(GenericValidation::kind),
            Some(GenericCredentialKind::ApiKey),
        );
    }

    #[test]
    fn rejects_unrelated_field_and_placeholder() {
        let value = "AbCdEfGhIjKlMnOpQrStUvWx";
        let source = format!("display_name={value}");
        assert!(validate_generic_credential(&context(&source, value)).is_none());

        let source = "API_KEY=your_api_key_here";
        assert!(validate_generic_credential(&context(source, "your_api_key_here")).is_none());
    }
}
