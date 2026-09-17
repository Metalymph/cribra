//! Contextual validation for Gradle repository credentials.
//!
//! Gradle project properties can be supplied through environment variables
//! using the documented `ORG_GRADLE_PROJECT_<property>` mapping. Repository
//! `PasswordCredentials` derive a `<repository>Username` and
//! `<repository>Password` property from the repository identity.
//!
//! Validation intentionally requires the explicit environment-variable form:
//! without source-path metadata, an arbitrary `<name>Password` assignment
//! cannot be attributed to Gradle reliably.

use super::context::ValidationContext;
use crate::validators::utils::is_obvious_placeholder;

const ENV_PREFIX: &str = "ORG_GRADLE_PROJECT_";
const PASSWORD_SUFFIX: &str = "Password";
const AUTH_HEADER_VALUE_SUFFIX: &str = "AuthHeaderValue";

/// Successful Gradle repository credential validation.
#[derive(Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub(crate) enum GradleCredentialKind {
    RepositoryPassword,
    RepositoryAuthHeaderValue,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct GradleValidation {
    kind: GradleCredentialKind,
}

impl GradleValidation {
    pub(crate) const fn kind(self) -> GradleCredentialKind {
        self.kind
    }
}

/// Validates a password supplied through a Gradle project-property environment
/// variable.
pub(crate) fn validate_gradle(context: &ValidationContext<'_>) -> Option<GradleValidation> {
    let candidate = context.candidate();

    if candidate.is_empty()
        || candidate.len() > 2048
        || candidate.chars().any(char::is_control)
        || is_gradle_placeholder(candidate)
    {
        return None;
    }

    let before = context.before_window(512);
    let line = before.rsplit_once('\n').map_or(before, |(_, line)| line);
    let assignment = line.trim_start();

    let property = assignment.strip_prefix(ENV_PREFIX)?;
    let property = property.strip_suffix('=')?.trim_end();

    if let Some(repository) = property.strip_suffix(PASSWORD_SUFFIX) {
        return (!repository.is_empty()).then_some(GradleValidation {
            kind: GradleCredentialKind::RepositoryPassword,
        });
    }

    if let Some(repository) = property.strip_suffix(AUTH_HEADER_VALUE_SUFFIX) {
        return (!repository.is_empty()).then_some(GradleValidation {
            kind: GradleCredentialKind::RepositoryAuthHeaderValue,
        });
    }

    None
}

fn is_gradle_placeholder(value: &str) -> bool {
    if is_obvious_placeholder(value) {
        return true;
    }

    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "password"
            | "changeme"
            | "replace_me"
            | "replace_me_here"
            | "your_password"
            | "your_password_here"
            | "example_password"
    )
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
    fn recognizes_repository_password_environment_property() {
        let password = "GradleRepositorySecret_123456";
        let source = format!("ORG_GRADLE_PROJECT_internalRepositoryPassword={password}");

        assert!(validate_gradle(&context(&source, password)).is_some());
    }

    #[test]
    fn rejects_empty_repository_identity() {
        let password = "GradleRepositorySecret_123456";
        let source = format!("ORG_GRADLE_PROJECT_Password={password}");

        assert!(validate_gradle(&context(&source, password)).is_none());
    }

    #[test]
    fn rejects_non_gradle_password_property() {
        let password = "GradleRepositorySecret_123456";

        for source in [
            format!("internalRepositoryPassword={password}"),
            format!("MY_ORG_GRADLE_PROJECT_internalRepositoryPassword={password}"),
            format!("ORG_GRADLE_PROJECT_internalRepositoryPasswordSuffix={password}"),
        ] {
            assert!(
                validate_gradle(&context(&source, password)).is_none(),
                "unrelated property unexpectedly validated",
            );
        }
    }

    #[test]
    fn rejects_placeholder_passwords() {
        for password in ["password", "changeme", "your_password"] {
            let source = format!("ORG_GRADLE_PROJECT_internalRepositoryPassword={password}");

            assert!(
                validate_gradle(&context(&source, password)).is_none(),
                "placeholder unexpectedly validated",
            );
        }
    }

    #[test]
    fn accepts_non_placeholder_passwords_with_similar_text() {
        for password in [
            "production_password_7f3a91",
            "GradleRepositorySecret_123456",
        ] {
            let source = format!("ORG_GRADLE_PROJECT_internalRepositoryPassword={password}");

            assert!(
                validate_gradle(&context(&source, password)).is_some(),
                "non-placeholder password unexpectedly rejected",
            );

            assert_eq!(
                validate_gradle(&context(&source, password)).map(GradleValidation::kind),
                Some(GradleCredentialKind::RepositoryPassword),
            );
        }
    }

    #[test]
    fn recognizes_repository_auth_header_value_environment_property() {
        let credential = "Bearer-GradleRepositoryToken_123456";
        let source = format!("ORG_GRADLE_PROJECT_internalRepositoryAuthHeaderValue={credential}");

        assert_eq!(
            validate_gradle(&context(&source, credential)).map(GradleValidation::kind),
            Some(GradleCredentialKind::RepositoryAuthHeaderValue),
        );
    }
}
