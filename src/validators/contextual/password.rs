//! Contextual validation for password-like configuration fields.
//!
//! Password values rarely have a unique standalone shape. Detection is based
//! on an explicit password-related field name plus conservative value checks.

use super::{
    context::ValidationContext,
    utils::{DEFAULT_KEY_WINDOW, key_matches_any, nearest_key},
};
use crate::validators::utils::is_obvious_placeholder;

/// Password-like configuration field recognized by contextual validation.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum PasswordKind {
    /// Generic password field.
    Password,

    /// Passphrase field.
    Passphrase,

    /// Database password field.
    DatabasePassword,
}

/// Successful password-field contextual validation.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct PasswordValidation {
    kind: PasswordKind,
}

impl PasswordValidation {
    pub(crate) const fn kind(self) -> PasswordKind {
        self.kind
    }
}

/// Validates a candidate assigned to a password-related configuration key.
pub(crate) fn validate_password(context: &ValidationContext<'_>) -> Option<PasswordValidation> {
    let candidate = context.candidate();

    if !valid_password_value(candidate) {
        return None;
    }

    let key = nearest_key(context.before_window(DEFAULT_KEY_WINDOW))?;

    let kind = if key_matches_any(
        key,
        &[
            "database_password",
            "db_password",
            "postgres_password",
            "mysql_password",
            "redis_password",
        ],
    ) {
        PasswordKind::DatabasePassword
    } else if key_matches_any(key, &["passphrase", "private_key_passphrase"]) {
        PasswordKind::Passphrase
    } else if key_matches_any(
        key,
        &[
            "password",
            "passwd",
            "pwd",
            "admin_password",
            "root_password",
        ],
    ) {
        PasswordKind::Password
    } else {
        return None;
    };

    Some(PasswordValidation { kind })
}

fn valid_password_value(candidate: &str) -> bool {
    (8..=1024).contains(&candidate.len())
        && !candidate.chars().any(char::is_control)
        && !is_obvious_placeholder(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context<'a>(source: &'a str, value: &'a str) -> ValidationContext<'a> {
        let start = source.find(value).expect("fixture must contain value");
        ValidationContext::new(source, start..start + value.len())
    }

    #[test]
    fn recognizes_password_and_database_password_fields() {
        let password = "CorrectHorseBatteryStaple!";
        let source = format!("password={password}");

        assert_eq!(
            validate_password(&context(&source, password)).map(PasswordValidation::kind),
            Some(PasswordKind::Password),
        );

        let source = format!("POSTGRES_PASSWORD={password}");
        assert_eq!(
            validate_password(&context(&source, password)).map(PasswordValidation::kind),
            Some(PasswordKind::DatabasePassword),
        );
    }

    #[test]
    fn rejects_placeholders_and_unrelated_fields() {
        let source = "password=changeme";
        assert!(validate_password(&context(source, "changeme")).is_none());

        let value = "CorrectHorseBatteryStaple!";
        let source = format!("display_name={value}");
        assert!(validate_password(&context(&source, value)).is_none());
    }
}
