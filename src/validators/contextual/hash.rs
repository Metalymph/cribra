//! Contextual classification of hexadecimal hash-like artifacts.
//!
//! Hashes are not automatically secrets. This validator only promotes a hash
//! candidate when a nearby field explicitly associates it with password,
//! secret or credential material.

use super::{
    context::ValidationContext,
    utils::{DEFAULT_KEY_WINDOW, key_matches_any, nearest_key},
};
use crate::validators::utils::is_obvious_placeholder;

/// Hexadecimal hash family inferred from candidate length.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum HashKind {
    Md5,
    Sha1,
    Sha256,
    Sha384,
    Sha512,
}

/// Successful contextual hash classification.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct HashValidation {
    kind: HashKind,
}

impl HashValidation {
    pub(crate) const fn kind(self) -> HashKind {
        self.kind
    }
}

/// Classifies a hexadecimal hash only when nearby context marks it as
/// sensitive.
pub(crate) fn validate_sensitive_hash(context: &ValidationContext<'_>) -> Option<HashValidation> {
    let candidate = context.candidate();

    if is_obvious_placeholder(candidate) || !candidate.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return None;
    }

    let key = nearest_key(context.before_window(DEFAULT_KEY_WINDOW))?;

    if !key_matches_any(
        key,
        &[
            "password_hash",
            "passwd_hash",
            "secret_hash",
            "credential_hash",
            "api_key_hash",
            "token_hash",
        ],
    ) {
        return None;
    }

    let kind = match candidate.len() {
        32 => HashKind::Md5,
        40 => HashKind::Sha1,
        64 => HashKind::Sha256,
        96 => HashKind::Sha384,
        128 => HashKind::Sha512,
        _ => return None,
    };

    Some(HashValidation { kind })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context<'a>(source: &'a str, value: &'a str) -> ValidationContext<'a> {
        let start = source.find(value).expect("fixture must contain value");
        ValidationContext::new(source, start..start + value.len())
    }

    #[test]
    fn classifies_sensitive_sha256_hash() {
        let value = "0123456789abcdef".repeat(4);
        let source = format!("password_hash={value}");

        assert_eq!(
            validate_sensitive_hash(&context(&source, &value)).map(HashValidation::kind),
            Some(HashKind::Sha256),
        );
    }

    #[test]
    fn ignores_generic_checksums() {
        let value = "0123456789abcdef".repeat(4);
        let source = format!("release_checksum={value}");

        assert!(validate_sensitive_hash(&context(&source, &value)).is_none());
    }
}
