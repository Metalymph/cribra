//! Contextual validation for Microsoft Azure and Entra application secrets.
//!
//! Client-secret values do not have one universally reliable public prefix.
//! Validation therefore requires recognized configuration keys and rejects
//! obvious examples, redactions and mock values.

use super::{
    context::ValidationContext,
    utils::{DEFAULT_KEY_WINDOW, key_matches_any, nearest_key},
};
use crate::validators::utils::is_obvious_placeholder;

/// Azure or Microsoft Entra secret family recognized by contextual validation.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum AzureCredentialKind {
    /// Application/client secret value.
    ClientSecret,

    /// Storage account key.
    StorageAccountKey,

    /// Shared access signature.
    SharedAccessSignature,
}

/// Successful Azure contextual validation.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct AzureValidation {
    kind: AzureCredentialKind,
}

impl AzureValidation {
    pub(crate) const fn kind(self) -> AzureCredentialKind {
        self.kind
    }
}

/// Validates an Azure or Microsoft Entra credential using nearby key context.
pub(crate) fn validate_azure(context: &ValidationContext<'_>) -> Option<AzureValidation> {
    let candidate = context.candidate();

    if candidate.is_empty() || !candidate.is_ascii() || is_obvious_placeholder(candidate) {
        return None;
    }

    let key = nearest_key(context.before_window(DEFAULT_KEY_WINDOW))?;

    if key_matches_any(
        key,
        &[
            "azure_client_secret",
            "client_secret",
            "clientsecret",
            "client_secret_value",
            "microsoft_provider_authentication_secret",
        ],
    ) && validate_client_secret(candidate)
    {
        return Some(AzureValidation {
            kind: AzureCredentialKind::ClientSecret,
        });
    }

    if key_matches_any(
        key,
        &["account_key", "storage_account_key", "azure_storage_key"],
    ) && validate_storage_key(candidate)
    {
        return Some(AzureValidation {
            kind: AzureCredentialKind::StorageAccountKey,
        });
    }

    if key_matches_any(
        key,
        &["shared_access_signature", "sas_token", "azure_sas_token"],
    ) && validate_sas(candidate)
    {
        return Some(AzureValidation {
            kind: AzureCredentialKind::SharedAccessSignature,
        });
    }

    None
}

fn validate_client_secret(candidate: &str) -> bool {
    (16..=255).contains(&candidate.len())
        && candidate.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'~' | b'.' | b'_' | b'-' | b'+' | b'/' | b'=')
        })
}

fn validate_storage_key(candidate: &str) -> bool {
    (40..=128).contains(&candidate.len())
        && candidate
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'='))
}

fn validate_sas(candidate: &str) -> bool {
    candidate.len() >= 32
        && candidate.contains("sig=")
        && (candidate.contains("sv=") || candidate.contains("se="))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context<'a>(source: &'a str, value: &'a str) -> ValidationContext<'a> {
        let start = source.find(value).expect("fixture must contain value");
        ValidationContext::new(source, start..start + value.len())
    }

    #[test]
    fn recognizes_client_secret_with_context() {
        let value = "AbCdEfGhIjKlMnOpQrStUvWxYz0123456789";
        let source = format!("AZURE_CLIENT_SECRET={value}");

        assert_eq!(
            validate_azure(&context(&source, value)).map(AzureValidation::kind),
            Some(AzureCredentialKind::ClientSecret),
        );
    }

    #[test]
    fn recognizes_storage_account_key() {
        let value = "AbCdEfGhIjKlMnOpQrStUvWxYz0123456789+/AbCdEfGhIjKlMnOpQrStUv==";
        let source = format!("AZURE_STORAGE_KEY={value}");

        assert_eq!(
            validate_azure(&context(&source, value)).map(AzureValidation::kind),
            Some(AzureCredentialKind::StorageAccountKey),
        );
    }

    #[test]
    fn rejects_secret_without_context_or_placeholder() {
        let value = "AbCdEfGhIjKlMnOpQrStUvWxYz0123456789";
        assert!(validate_azure(&context(value, value)).is_none());

        let source = "AZURE_CLIENT_SECRET=your_api_key_here";
        assert!(validate_azure(&context(source, "your_api_key_here")).is_none());
    }
}
