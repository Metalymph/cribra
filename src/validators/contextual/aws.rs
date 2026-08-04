//! Contextual validation for AWS credentials.
//!
//! AWS access key IDs have recognizable prefixes, while secret access keys are
//! not sufficiently distinctive by value alone. This module therefore combines
//! candidate shape with nearby configuration-key context.

use super::{
    context::ValidationContext,
    utils::{DEFAULT_KEY_WINDOW, key_matches_any, nearest_key},
};
use crate::validators::utils::is_obvious_placeholder;

const ACCESS_KEY_ID_LEN: usize = 20;
const SECRET_ACCESS_KEY_LEN: usize = 40;

/// AWS credential family recognized by contextual validation.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum AwsCredentialKind {
    /// Long-term access key ID beginning with `AKIA`.
    LongTermAccessKeyId,

    /// Temporary STS access key ID beginning with `ASIA`.
    TemporaryAccessKeyId,

    /// Secret access key associated with an access key ID.
    SecretAccessKey,

    /// Temporary session token found under a recognized AWS session-token key.
    SessionToken,
}

/// Successful AWS contextual validation.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct AwsValidation {
    kind: AwsCredentialKind,
}

impl AwsValidation {
    pub(crate) const fn kind(self) -> AwsCredentialKind {
        self.kind
    }
}

/// Validates an AWS credential candidate using its value and nearby key name.
pub(crate) fn validate_aws(context: &ValidationContext<'_>) -> Option<AwsValidation> {
    let candidate = context.candidate();

    if is_obvious_placeholder(candidate) || !candidate.is_ascii() {
        return None;
    }

    if validate_access_key_id(candidate, "AKIA") {
        return Some(AwsValidation {
            kind: AwsCredentialKind::LongTermAccessKeyId,
        });
    }

    if validate_access_key_id(candidate, "ASIA") {
        return Some(AwsValidation {
            kind: AwsCredentialKind::TemporaryAccessKeyId,
        });
    }

    let key = nearest_key(context.before_window(DEFAULT_KEY_WINDOW))?;

    if key_matches_any(
        key,
        &[
            "aws_secret_access_key",
            "secret_access_key",
            "aws_secret_key",
        ],
    ) && validate_secret_access_key(candidate)
    {
        return Some(AwsValidation {
            kind: AwsCredentialKind::SecretAccessKey,
        });
    }

    if key_matches_any(
        key,
        &["aws_session_token", "session_token", "aws_security_token"],
    ) && validate_session_token(candidate)
    {
        return Some(AwsValidation {
            kind: AwsCredentialKind::SessionToken,
        });
    }

    None
}

fn validate_access_key_id(candidate: &str, prefix: &str) -> bool {
    candidate.len() == ACCESS_KEY_ID_LEN
        && candidate.starts_with(prefix)
        && candidate
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
}

fn validate_secret_access_key(candidate: &str) -> bool {
    candidate.len() == SECRET_ACCESS_KEY_LEN
        && candidate
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'+' | b'='))
}

fn validate_session_token(candidate: &str) -> bool {
    candidate.len() >= 80
        && candidate.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'+' | b'=' | b'_' | b'-')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context<'a>(source: &'a str, value: &'a str) -> ValidationContext<'a> {
        let start = source.find(value).expect("fixture must contain value");
        ValidationContext::new(source, start..start + value.len())
    }

    #[test]
    fn recognizes_long_term_and_temporary_access_key_ids() {
        let long_term = "AKIAIOSFODNN7EXAMPLE";
        let temporary = "ASIAIOSFODNN7EXAMPLE";

        assert_eq!(
            validate_aws(&context(long_term, long_term)).map(AwsValidation::kind),
            Some(AwsCredentialKind::LongTermAccessKeyId),
        );
        assert_eq!(
            validate_aws(&context(temporary, temporary)).map(AwsValidation::kind),
            Some(AwsCredentialKind::TemporaryAccessKeyId),
        );
    }

    #[test]
    fn requires_context_for_secret_access_keys() {
        let value = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY";
        let source = format!("AWS_SECRET_ACCESS_KEY={value}");

        assert_eq!(
            validate_aws(&context(&source, value)).map(AwsValidation::kind),
            Some(AwsCredentialKind::SecretAccessKey),
        );
        assert!(validate_aws(&context(value, value)).is_none());
    }

    #[test]
    fn rejects_placeholder_secret() {
        let source = "AWS_SECRET_ACCESS_KEY=your_api_key_here";
        assert!(validate_aws(&context(source, "your_api_key_here")).is_none());
    }
}
