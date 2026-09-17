//! Contextual validation for Swift Package Manager credentials.
//!
//! SwiftPM exposes credentials through explicitly named environment variables.
//! Validation binds candidates to those documented credential-bearing
//! variables rather than attempting to classify their values heuristically.

use super::context::ValidationContext;
use crate::validators::utils::is_obvious_placeholder;

/// Successful SwiftPM credential validation.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct SwiftPmValidation;

/// Validates a credential candidate belonging to a supported SwiftPM
/// environment variable.
///
/// Candidate discovery establishes the concrete variable name. This validator
/// independently checks that the candidate belongs to that assignment and
/// rejects empty, control-bearing, and obvious placeholder values.
pub(crate) fn validate_swiftpm(context: &ValidationContext<'_>) -> Option<SwiftPmValidation> {
    let candidate = context.candidate();

    if candidate.is_empty()
        || candidate.len() > 16 * 1024
        || candidate.chars().any(char::is_control)
        || is_swiftpm_placeholder(candidate)
    {
        return None;
    }

    let before = context.before_window(256);
    let line = before.rsplit_once('\n').map_or(before, |(_, line)| line);

    let assignment = line.trim_start();

    [
        "SWIFTPM_REGISTRY_TOKEN",
        "SWIFTPM_REGISTRY_PASSWORD",
        "SWIFTPM_SOURCE_CONTROL_TOKEN",
    ]
    .iter()
    .any(|name| {
        assignment
            .strip_prefix(name)
            .is_some_and(|rest| rest.trim_start().starts_with('='))
    })
    .then_some(SwiftPmValidation)
}

fn is_swiftpm_placeholder(value: &str) -> bool {
    if is_obvious_placeholder(value) {
        return true;
    }

    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "replace_me"
            | "replace_me_here"
            | "your_password"
            | "your_password_here"
            | "example_password"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_swiftpm_documentation_placeholders() {
        for value in [
            "replace_me",
            "REPLACE_ME",
            "replace_me_here",
            "your_password",
            "your_password_here",
            "example_password",
        ] {
            assert!(is_swiftpm_placeholder(value), "{value}");
        }
    }

    #[test]
    fn accepts_non_placeholder_values_with_similar_text() {
        for value in [
            "replace_me_now_7f3a91",
            "production_password_7f3a91",
            "SwiftPMRegistryPassword_123456",
        ] {
            assert!(!is_swiftpm_placeholder(value), "{value}");
        }
    }
}
