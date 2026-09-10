//! Contextual validation for credentials embedded in database connection URIs.
//!
//! Detection requires a recognized database URI scheme and explicit
//! `username:password@host` userinfo. Only the password is projected as the
//! final finding.

use super::context::ValidationContext;
use crate::validators::utils::is_obvious_placeholder;

const MAX_PASSWORD_LEN: usize = 1024;
const SCHEME_WINDOW: usize = 256;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum DatabaseConnectionKind {
    PostgreSql,
    MySql,
    MariaDb,
    MongoDb,
    Redis,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct DatabaseConnectionValidation {
    kind: DatabaseConnectionKind,
}

impl DatabaseConnectionValidation {
    pub(crate) const fn kind(self) -> DatabaseConnectionKind {
        self.kind
    }
}

pub(crate) fn validate_database_connection(
    context: &ValidationContext<'_>,
) -> Option<DatabaseConnectionValidation> {
    let password = context.candidate();

    if password.is_empty()
        || password.len() > MAX_PASSWORD_LEN
        || password.chars().any(char::is_control)
        || is_obvious_placeholder(password)
        || !valid_percent_encoding(password)
    {
        return None;
    }

    let before = context.before_window(SCHEME_WINDOW);
    let lower = before.to_ascii_lowercase();

    let kind = if has_connection_prefix(&lower, &["postgres://", "postgresql://"]) {
        DatabaseConnectionKind::PostgreSql
    } else if has_connection_prefix(&lower, &["mysql://"]) {
        DatabaseConnectionKind::MySql
    } else if has_connection_prefix(&lower, &["mariadb://"]) {
        DatabaseConnectionKind::MariaDb
    } else if has_connection_prefix(&lower, &["mongodb://", "mongodb+srv://"]) {
        DatabaseConnectionKind::MongoDb
    } else if has_connection_prefix(&lower, &["redis://", "rediss://"]) {
        DatabaseConnectionKind::Redis
    } else {
        return None;
    };

    Some(DatabaseConnectionValidation { kind })
}

fn has_connection_prefix(before: &str, schemes: &[&str]) -> bool {
    schemes.iter().any(|scheme| {
        before.rfind(scheme).is_some_and(|start| {
            let userinfo = &before[start + scheme.len()..];
            !userinfo.is_empty() && userinfo.ends_with(':')
        })
    })
}

fn valid_percent_encoding(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len()
                || !bytes[index + 1].is_ascii_hexdigit()
                || !bytes[index + 2].is_ascii_hexdigit()
            {
                return false;
            }

            index += 3;
        } else {
            index += 1;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context<'a>(source: &'a str, password: &'a str) -> ValidationContext<'a> {
        let start = source
            .find(password)
            .expect("fixture must contain password");
        ValidationContext::new(source, start..start + password.len())
    }

    #[test]
    fn recognizes_supported_database_connection_schemes() {
        for (source, password, expected) in [
            (
                "postgresql://alice:CorrectHorseBatteryStaple@localhost/app",
                "CorrectHorseBatteryStaple",
                DatabaseConnectionKind::PostgreSql,
            ),
            (
                "mysql://alice:CorrectHorseBatteryStaple@localhost/app",
                "CorrectHorseBatteryStaple",
                DatabaseConnectionKind::MySql,
            ),
            (
                "mariadb://alice:CorrectHorseBatteryStaple@localhost/app",
                "CorrectHorseBatteryStaple",
                DatabaseConnectionKind::MariaDb,
            ),
            (
                "mongodb+srv://alice:CorrectHorseBatteryStaple@cluster.example/app",
                "CorrectHorseBatteryStaple",
                DatabaseConnectionKind::MongoDb,
            ),
            (
                "rediss://alice:CorrectHorseBatteryStaple@cache.example:6379",
                "CorrectHorseBatteryStaple",
                DatabaseConnectionKind::Redis,
            ),
        ] {
            assert_eq!(
                validate_database_connection(&context(source, password))
                    .map(DatabaseConnectionValidation::kind),
                Some(expected),
            );
        }
    }

    #[test]
    fn accepts_valid_percent_encoded_passwords() {
        let password = "Correct%40Horse%2FBattery";
        let source = format!("postgres://alice:{password}@localhost/app");

        assert_eq!(
            validate_database_connection(&context(&source, password))
                .map(DatabaseConnectionValidation::kind),
            Some(DatabaseConnectionKind::PostgreSql),
        );
    }

    #[test]
    fn rejects_placeholders_and_malformed_percent_encoding() {
        for (source, password) in [
            ("postgres://alice:changeme@localhost/app", "changeme"),
            ("postgres://alice:secret%2@localhost/app", "secret%2"),
            ("postgres://alice:secret%XX@localhost/app", "secret%XX"),
        ] {
            assert!(validate_database_connection(&context(source, password)).is_none());
        }
    }
}
