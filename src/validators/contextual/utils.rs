//! Shared helpers for contextual validators.
//!
//! These helpers inspect small borrowed source windows and allocate nothing.

/// Maximum source window inspected before a candidate for key-name context.
pub(crate) const DEFAULT_KEY_WINDOW: usize = 160;

/// Returns the nearest identifier-like key before the candidate.
///
/// The helper supports common configuration forms such as:
///
/// - `KEY=value`
/// - `key: value`
/// - `"key": "value"`
/// - `key = "value"`
///
/// The returned slice borrows from `before`.
pub(crate) fn nearest_key(before: &str) -> Option<&str> {
    let trimmed = before.trim_end_matches(|character: char| {
        character.is_ascii_whitespace() || matches!(character, '"' | '\'' | '`')
    });

    let separator = trimmed
        .char_indices()
        .rev()
        .find(|(_, character)| matches!(character, '=' | ':'))?
        .0;

    let left = trimmed[..separator].trim_end_matches(|character: char| {
        character.is_ascii_whitespace() || matches!(character, '"' | '\'' | '`')
    });

    let end = left.len();
    let start = left
        .char_indices()
        .rev()
        .find(|(_, character)| !is_key_character(*character))
        .map_or(0, |(index, character)| index + character.len_utf8());

    let key = &left[start..end];
    (!key.is_empty()).then_some(key)
}

/// Compares a configuration key using ASCII case-insensitive matching while
/// treating `-`, `_` and `.` as equivalent separators.
pub(crate) fn key_matches(key: &str, expected: &str) -> bool {
    normalized_key_bytes(key).eq(normalized_key_bytes(expected))
}

/// Returns `true` when `key` matches at least one expected configuration key.
pub(crate) fn key_matches_any(key: &str, expected: &[&str]) -> bool {
    expected.iter().any(|candidate| key_matches(key, candidate))
}

/// Returns `true` when the source window contains an ASCII case-insensitive
/// keyword.
pub(crate) fn contains_ascii_case_insensitive(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }

    haystack
        .as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

fn normalized_key_bytes(value: &str) -> impl Iterator<Item = u8> + '_ {
    value.bytes().map(|byte| match byte {
        b'-' | b'.' => b'_',
        _ => byte.to_ascii_lowercase(),
    })
}

fn is_key_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_common_configuration_keys() {
        assert_eq!(
            nearest_key("AWS_SECRET_ACCESS_KEY="),
            Some("AWS_SECRET_ACCESS_KEY")
        );
        assert_eq!(nearest_key(r#""private_key": ""#), Some("private_key"));
        assert_eq!(nearest_key("client-secret: "), Some("client-secret"));
    }

    #[test]
    fn normalizes_key_separators_and_case() {
        assert!(key_matches("AZURE-CLIENT.SECRET", "azure_client_secret"));
        assert!(key_matches_any("password", &["passwd", "password"]));
    }

    #[test]
    fn finds_keywords_without_allocating() {
        assert!(contains_ascii_case_insensitive(
            "BEGIN PRIVATE KEY",
            "private key",
        ));
    }
}
