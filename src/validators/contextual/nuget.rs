//! Contextual validation for NuGet package-source credentials.
//!
//! NuGet configuration uses ClearTextPassword for an unencrypted package
//! source password. This validator checks only the bounded XML region needed
//! for that contract; it does not parse XML.

use super::ValidationContext;
use crate::validators::utils::is_obvious_placeholder;

const CONTEXT_WINDOW: usize = 16 * 1024;
const OPEN_TAG: &[u8] = b"<packagesourcecredentials";
const CLOSE_TAG: &[u8] = b"</packagesourcecredentials";

/// Successful NuGet package-source cleartext-password validation.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct NugetValidation;

/// Validates a candidate captured from a NuGet ClearTextPassword attribute.
pub(crate) fn validate_nuget(context: &ValidationContext<'_>) -> Option<NugetValidation> {
    let candidate = context.candidate();

    if candidate.is_empty()
        || candidate.chars().any(char::is_control)
        || is_obvious_placeholder(candidate)
    {
        return None;
    }

    let before = context.before_window(CONTEXT_WINDOW);
    let after = context.after_window(CONTEXT_WINDOW);

    if !inside_credentials_region(before)
        || !has_closing_credentials_tag(after)
        || is_inside_unsupported_markup(before)
    {
        return None;
    }

    Some(NugetValidation)
}

fn inside_credentials_region(source: &str) -> bool {
    let bytes = source.as_bytes();
    let mut active = false;
    let mut cursor = 0;

    while cursor < bytes.len() {
        if starts_with_ascii_case_insensitive(bytes, cursor, b"<!--") {
            let Some(next) = skip_delimited(bytes, cursor + 4, b"-->") else {
                return false;
            };
            cursor = next;
            continue;
        }

        if starts_with_ascii_case_insensitive(bytes, cursor, b"<![cdata[") {
            let Some(next) = skip_delimited(bytes, cursor + 9, b"]]>") else {
                return false;
            };
            cursor = next;
            continue;
        }

        if starts_with_ascii_case_insensitive(bytes, cursor, CLOSE_TAG)
            && tag_name_boundary(bytes, cursor + CLOSE_TAG.len())
        {
            let Some(next) = tag_end(bytes, cursor + CLOSE_TAG.len()) else {
                return false;
            };
            cursor = next;
            active = false;
            continue;
        }

        if starts_with_ascii_case_insensitive(bytes, cursor, OPEN_TAG)
            && tag_name_boundary(bytes, cursor + OPEN_TAG.len())
        {
            let Some(end) = tag_end(bytes, cursor + OPEN_TAG.len()) else {
                return false;
            };
            active = !is_self_closing_tag(&bytes[cursor..=end]);
            cursor = end + 1;
            continue;
        }

        cursor += 1;
    }

    active
}

fn has_closing_credentials_tag(source: &str) -> bool {
    let bytes = source.as_bytes();
    let mut cursor = 0;

    while cursor < bytes.len() {
        if starts_with_ascii_case_insensitive(bytes, cursor, b"<!--") {
            let Some(next) = skip_delimited(bytes, cursor + 4, b"-->") else {
                return false;
            };
            cursor = next;
            continue;
        }

        if starts_with_ascii_case_insensitive(bytes, cursor, b"<![cdata[") {
            let Some(next) = skip_delimited(bytes, cursor + 9, b"]]>") else {
                return false;
            };
            cursor = next;
            continue;
        }

        if starts_with_ascii_case_insensitive(bytes, cursor, CLOSE_TAG)
            && tag_name_boundary(bytes, cursor + CLOSE_TAG.len())
        {
            return tag_end(bytes, cursor + CLOSE_TAG.len()).is_some();
        }

        cursor += 1;
    }

    false
}

fn is_inside_unsupported_markup(source: &str) -> bool {
    let bytes = source.as_bytes();
    let mut cursor = 0;

    while cursor < bytes.len() {
        if starts_with_ascii_case_insensitive(bytes, cursor, b"<!--") {
            let Some(next) = skip_delimited(bytes, cursor + 4, b"-->") else {
                return true;
            };
            cursor = next;
            continue;
        }

        if starts_with_ascii_case_insensitive(bytes, cursor, b"<![cdata[") {
            let Some(next) = skip_delimited(bytes, cursor + 9, b"]]>") else {
                return true;
            };
            cursor = next;
            continue;
        }

        cursor += 1;
    }

    false
}

fn starts_with_ascii_case_insensitive(source: &[u8], offset: usize, needle: &[u8]) -> bool {
    source
        .get(offset..offset.saturating_add(needle.len()))
        .is_some_and(|window| window.eq_ignore_ascii_case(needle))
}

fn tag_name_boundary(source: &[u8], offset: usize) -> bool {
    source
        .get(offset)
        .is_none_or(|byte| !byte.is_ascii_alphanumeric() && !matches!(byte, b'_' | b'-' | b':'))
}

fn tag_end(source: &[u8], mut offset: usize) -> Option<usize> {
    while let Some(byte) = source.get(offset) {
        if *byte == b'>' {
            return Some(offset);
        }
        offset += 1;
    }

    None
}

fn skip_delimited(source: &[u8], mut offset: usize, delimiter: &[u8]) -> Option<usize> {
    while offset < source.len() {
        if starts_with_ascii_case_insensitive(source, offset, delimiter) {
            return Some(offset + delimiter.len());
        }
        offset += 1;
    }

    None
}

fn is_self_closing_tag(tag: &[u8]) -> bool {
    tag[..tag.len().saturating_sub(1)]
        .iter()
        .rev()
        .find(|byte| !byte.is_ascii_whitespace())
        .is_some_and(|byte| *byte == b'/')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validators::contextual::ValidationContext;

    fn accepted(source: &str, value: &str) -> bool {
        let start = source.find(value).expect("fixture must contain value");
        validate_nuget(&ValidationContext::new(source, start..start + value.len())).is_some()
    }

    #[test]
    fn accepts_a_complete_credentials_region() {
        let source = r#"<packageSourceCredentials>
  <Feed><add key="ClearTextPassword" value="secret-value" /></Feed>
</packageSourceCredentials>"#;

        assert!(accepted(source, "secret-value"));
    }

    #[test]
    fn rejects_placeholder_values() {
        let placeholder = r#"<packageSourceCredentials><Feed><add key="ClearTextPassword" value="placeholder_value" /></Feed></packageSourceCredentials>"#;

        assert!(!accepted(placeholder, "placeholder_value"));
    }

    #[test]
    fn rejects_context_outside_or_inside_comments() {
        let outside = r#"<Feed><add key="ClearTextPassword" value="secret-value" /></Feed>"#;
        let comment = r#"<!-- <packageSourceCredentials><Feed><add key="ClearTextPassword" value="secret-value" /></Feed></packageSourceCredentials> -->"#;

        assert!(!accepted(outside, "secret-value"));
        assert!(!accepted(comment, "secret-value"));
    }
}
