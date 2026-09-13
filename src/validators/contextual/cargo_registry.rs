//! Contextual validation for Cargo registry authentication tokens.
//!
//! Cargo registry tokens are accepted only when they are associated with
//! Cargo's documented registry credential surfaces:
//!
//! - `[registry] token = "..."`
//! - `[registries.<name>] token = "..."`
//! - `CARGO_REGISTRY_TOKEN=...`
//! - `CARGO_REGISTRIES_<NAME>_TOKEN=...`
//!
//! No registry network access is performed and token activity is never
//! inferred.

use super::{
    ValidationContext,
    utils::{DEFAULT_KEY_WINDOW, key_matches, nearest_key},
};
use crate::validators::utils::is_obvious_placeholder;

const TOML_SECTION_WINDOW: usize = 4096;

/// Successful Cargo registry credential validation.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct CargoRegistryValidation;

/// Validates one candidate as Cargo registry authentication material.
pub(crate) fn validate_cargo_registry(
    context: &ValidationContext<'_>,
) -> Option<CargoRegistryValidation> {
    let candidate = context.candidate();

    if candidate.is_empty()
        || candidate.len() > 2048
        || candidate.chars().any(char::is_control)
        || is_obvious_placeholder(candidate)
    {
        return None;
    }

    if is_cargo_registry_environment(context) || is_cargo_registry_toml(context) {
        Some(CargoRegistryValidation)
    } else {
        None
    }
}

fn is_cargo_registry_environment(context: &ValidationContext<'_>) -> bool {
    let before = context.before_window(DEFAULT_KEY_WINDOW);
    let Some(key) = nearest_key(before) else {
        return false;
    };

    key_matches(key, "cargo_registry_token") || is_named_registry_environment_key(key)
}

fn is_named_registry_environment_key(key: &str) -> bool {
    const PREFIX: &str = "cargo_registries_";
    const SUFFIX: &str = "_token";

    let bytes = key.as_bytes();

    if bytes.len() <= PREFIX.len() + SUFFIX.len() {
        return false;
    }

    bytes[..PREFIX.len()].eq_ignore_ascii_case(PREFIX.as_bytes())
        && bytes[bytes.len() - SUFFIX.len()..].eq_ignore_ascii_case(SUFFIX.as_bytes())
}

fn is_cargo_registry_toml(context: &ValidationContext<'_>) -> bool {
    let key_window = context.before_window(DEFAULT_KEY_WINDOW);

    if nearest_key(key_window).is_none_or(|key| !key_matches(key, "token")) {
        return false;
    }

    let before = context.before_window(TOML_SECTION_WINDOW);
    let Some(section) = last_toml_section(before) else {
        return false;
    };

    if section.eq_ignore_ascii_case("registry") {
        return true;
    }

    let Some(name) = strip_ascii_case_insensitive_prefix(section, "registries.") else {
        return false;
    };

    !name.trim().is_empty()
}

fn last_toml_section(source: &str) -> Option<&str> {
    source.lines().rev().find_map(|line| {
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') {
            return None;
        }

        line.strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
            .map(str::trim)
    })
}

fn strip_ascii_case_insensitive_prefix<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    let prefix_len = prefix.len();

    if value.len() < prefix_len
        || !value.as_bytes()[..prefix_len].eq_ignore_ascii_case(prefix.as_bytes())
    {
        return None;
    }

    Some(&value[prefix_len..])
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
    fn recognizes_default_registry_token() {
        let token = "cargo-secret-token-0123456789";
        let source = format!("[registry]\ntoken = \"{token}\"");

        assert!(validate_cargo_registry(&context(&source, token)).is_some());
    }

    #[test]
    fn recognizes_named_registry_token() {
        let token = "private-registry-secret-0123456789";
        let source = format!(
            "[registries.internal]\n\
             index = \"https://example.invalid/index\"\n\
             token = \"{token}\""
        );

        assert!(validate_cargo_registry(&context(&source, token)).is_some());
    }

    #[test]
    fn recognizes_default_registry_environment_token() {
        let token = "cargo-env-secret-0123456789";
        let source = format!("CARGO_REGISTRY_TOKEN={token}");

        assert!(validate_cargo_registry(&context(&source, token)).is_some());
    }

    #[test]
    fn recognizes_named_registry_environment_token() {
        let token = "cargo-private-env-secret-0123456789";
        let source = format!("CARGO_REGISTRIES_INTERNAL_TOKEN={token}");

        assert!(validate_cargo_registry(&context(&source, token)).is_some());
    }

    #[test]
    fn rejects_unrelated_token_field() {
        let token = "ordinary-secret-token-0123456789";

        for source in [
            format!("token = \"{token}\""),
            format!("[package]\ntoken = \"{token}\""),
            format!("[profile.release]\ntoken = \"{token}\""),
        ] {
            assert!(
                validate_cargo_registry(&context(&source, token)).is_none(),
                "unexpected Cargo credential validation",
            );
        }
    }

    #[test]
    fn rejects_malformed_named_registry_environment_key() {
        let token = "cargo-secret-token-0123456789";

        for source in [
            format!("CARGO_REGISTRIES__TOKEN={token}"),
            format!("CARGO_REGISTRIES_TOKEN={token}"),
        ] {
            assert!(
                validate_cargo_registry(&context(&source, token)).is_none(),
                "malformed Cargo registry environment key unexpectedly validated",
            );
        }
    }

    #[test]
    fn rejects_placeholder_values() {
        for token in ["token", "your_token", "your_token_here", "example_token"] {
            let source = format!("[registry]\ntoken = \"{token}\"");

            assert!(
                validate_cargo_registry(&context(&source, token)).is_none(),
                "unexpected Cargo placeholder validation",
            );
        }
    }
}
