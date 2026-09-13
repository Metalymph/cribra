//! Contextual validation for PyPI-style repository authentication tokens.
//!
//! PyPI API tokens are conventionally stored in `.pypirc` repository sections
//! as:
//!
//! `username = __token__`
//! `password = <token>`
//!
//! Cribra recognizes only that strong authentication contract. Ordinary
//! repository username/password pairs remain covered by generic password
//! detection rather than being classified as PyPI-specific credentials.

use super::{
    ValidationContext,
    utils::{DEFAULT_KEY_WINDOW, key_matches, nearest_key},
};
use crate::validators::utils::is_obvious_placeholder;

const SECTION_WINDOW: usize = 4096;
const MIN_TOKEN_LEN: usize = 8;
const MAX_TOKEN_LEN: usize = 2048;

/// Successful PyPI repository-token validation.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct PypiValidation;

/// Validates a candidate as a PyPI-style repository token.
pub(crate) fn validate_pypi(context: &ValidationContext<'_>) -> Option<PypiValidation> {
    let candidate = context.candidate();

    if !(MIN_TOKEN_LEN..=MAX_TOKEN_LEN).contains(&candidate.len())
        || candidate.chars().any(char::is_control)
        || is_obvious_placeholder(candidate)
        || is_documentation_value(candidate)
    {
        return None;
    }

    let key = nearest_key(context.before_window(DEFAULT_KEY_WINDOW))?;

    if !key_matches(key, "password") {
        return None;
    }

    let before = context.before_window(SECTION_WINDOW);
    let (section, body) = current_ini_section(before)?;

    if !has_token_username(body) {
        return None;
    }

    if is_default_pypi_section(section) || has_repository_url(body) {
        Some(PypiValidation)
    } else {
        None
    }
}

fn current_ini_section(source: &str) -> Option<(&str, &str)> {
    let mut offset = 0;
    let mut current = None;

    for line in source.split_inclusive('\n') {
        let logical = line.trim_end_matches(['\r', '\n']);
        let trimmed = logical.trim();

        if let Some(section) = trimmed
            .strip_prefix('[')
            .and_then(|value| value.strip_suffix(']'))
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            current = Some((section, offset + line.len()));
        }

        offset += line.len();
    }

    let (section, body_start) = current?;

    Some((section, &source[body_start..]))
}

fn has_token_username(body: &str) -> bool {
    body.lines().any(|line| {
        assignment(line)
            .is_some_and(|(key, value)| key_matches(key, "username") && value == "__token__")
    })
}

fn has_repository_url(body: &str) -> bool {
    body.lines().any(|line| {
        assignment(line).is_some_and(|(key, value)| {
            key_matches(key, "repository")
                && (starts_with_ascii_case_insensitive(value, "https://")
                    || starts_with_ascii_case_insensitive(value, "http://"))
        })
    })
}

fn assignment(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim();

    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
        return None;
    }

    let (key, value) = trimmed.split_once('=')?;

    let key = key.trim();
    let value = value.trim();

    (!key.is_empty() && !value.is_empty()).then_some((key, value))
}

fn is_default_pypi_section(section: &str) -> bool {
    section.eq_ignore_ascii_case("pypi") || section.eq_ignore_ascii_case("testpypi")
}

fn starts_with_ascii_case_insensitive(value: &str, prefix: &str) -> bool {
    value
        .as_bytes()
        .get(..prefix.len())
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(prefix.as_bytes()))
}

fn is_documentation_value(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "password"
            | "token"
            | "pypi_token"
            | "your_token"
            | "your_token_here"
            | "example_token"
            | "replace_me"
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
    fn recognizes_default_pypi_token_section() {
        let token = "pypi-AbCdEfGhIjKlMnOpQrStUvWxYz012345";
        let source = format!(
            "[pypi]\n\
             username = __token__\n\
             password = {token}\n"
        );

        assert!(validate_pypi(&context(&source, token)).is_some());
    }

    #[test]
    fn recognizes_testpypi_token_section() {
        let token = "pypi-TestTokenAbCdEfGhIjKlMnOp012345";
        let source = format!(
            "[testpypi]\n\
             username = __token__\n\
             password = {token}\n"
        );

        assert!(validate_pypi(&context(&source, token)).is_some());
    }

    #[test]
    fn recognizes_custom_repository_token_section() {
        let token = "private-package-token-0123456789";
        let source = format!(
            "[internal]\n\
             repository = https://packages.example.invalid/legacy/\n\
             username = __token__\n\
             password = {token}\n"
        );

        assert!(validate_pypi(&context(&source, token)).is_some());
    }

    #[test]
    fn rejects_unrelated_password_sections() {
        let token = "ordinary-secret-value-0123456789";

        for source in [
            format!(
                "[application]\n\
                 username = __token__\n\
                 password = {token}\n"
            ),
            format!(
                "[internal]\n\
                 username = alice\n\
                 password = {token}\n"
            ),
            format!(
                "[internal]\n\
                 repository = https://packages.example.invalid/\n\
                 username = alice\n\
                 password = {token}\n"
            ),
        ] {
            assert!(
                validate_pypi(&context(&source, token)).is_none(),
                "unrelated password unexpectedly validated as PyPI token",
            );
        }
    }

    #[test]
    fn does_not_cross_ini_section_boundaries() {
        let token = "private-package-token-0123456789";
        let source = format!(
            "[first]\n\
             repository = https://packages.example.invalid/\n\
             username = __token__\n\
             [second]\n\
             password = {token}\n"
        );

        assert!(validate_pypi(&context(&source, token)).is_none());
    }

    #[test]
    fn rejects_placeholders() {
        for token in [
            "your_token",
            "your_token_here",
            "example_token",
            "replace_me",
        ] {
            let source = format!(
                "[pypi]\n\
                 username = __token__\n\
                 password = {token}\n"
            );

            assert!(
                validate_pypi(&context(&source, token)).is_none(),
                "documentation token unexpectedly validated",
            );
        }
    }
}
