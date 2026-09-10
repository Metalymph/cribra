//! Contextual validation for Docker registry credentials.
//!
//! Docker stores registry authentication material in `config.json` under
//! `auths.<registry>.auth` as standard Base64 encoding of `username:password`.
//! The encoded source value is retained as the finding span; decoded material
//! is used only transiently for local structural validation.

use base64::{Engine as _, engine::general_purpose::STANDARD};

use super::{
    context::ValidationContext,
    utils::{key_matches_any, nearest_key},
};
use crate::validators::utils::is_obvious_placeholder;

const CONTEXT_WINDOW: usize = 16 * 1024;
const MAX_ENCODED_LEN: usize = 4096;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct DockerRegistryValidation;

pub(crate) fn validate_docker_registry(
    context: &ValidationContext<'_>,
) -> Option<DockerRegistryValidation> {
    let candidate = context.candidate();

    if !(4..=MAX_ENCODED_LEN).contains(&candidate.len()) || !candidate.is_ascii() {
        return None;
    }

    let before = context.before_window(CONTEXT_WINDOW);
    let key = nearest_key(before)?;

    if !key_matches_any(key, &["auth"]) || !is_docker_auths_entry(before) {
        return None;
    }

    let decoded = STANDARD.decode(candidate).ok()?;

    // Require canonical standard Base64. This rejects malformed padding and
    // non-canonical representations.
    if STANDARD.encode(&decoded) != candidate {
        return None;
    }

    let separator = decoded.iter().position(|byte| *byte == b':')?;
    let username = &decoded[..separator];
    let password = &decoded[separator + 1..];

    if username.is_empty() || password.is_empty() {
        return None;
    }

    if username.iter().any(u8::is_ascii_control) || password.iter().any(u8::is_ascii_control) {
        return None;
    }

    let username_text = std::str::from_utf8(username).ok();
    let password_text = std::str::from_utf8(password).ok();

    if username_text.is_some_and(is_obvious_placeholder)
        || password_text.is_some_and(is_obvious_placeholder)
    {
        return None;
    }

    // Reject common documentation/example pairs locally without broadening
    // the global placeholder policy.
    if matches!(
        (username_text, password_text),
        (Some("user" | "username"), Some("password" | "passwd"))
    ) {
        return None;
    }

    Some(DockerRegistryValidation)
}

/// Returns true only when the candidate's `auth` field is inside the shape:
///
/// `"auths": { "<registry>": { "auth": ... } }`
///
/// This is intentionally a small structural check rather than a general JSON
/// parser. Braces inside JSON strings are ignored.
fn is_docker_auths_entry(before: &str) -> bool {
    let Some(auths_key) = find_last_json_key(before, "auths") else {
        return false;
    };

    let tail = &before[auths_key..];

    let Some(auths_object_start) = find_object_start_after_key(tail) else {
        return false;
    };

    let structure = &tail[auths_object_start..];

    // The captured credential begins inside a JSON string, so `before` ends
    // immediately after the quote opening the `auth` value. Remove precisely
    // that quote before measuring object depth.
    let Some(structure) = structure.trim_end().strip_suffix('"') else {
        return false;
    };

    // At an `auth` value we must be exactly two levels beneath the `auths`
    // object: `auths` itself and one registry-entry object.
    json_object_depth(structure) == Some(2)
}

fn find_last_json_key(source: &str, expected: &str) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut index = 0;
    let mut last = None;

    while index < bytes.len() {
        if bytes[index] != b'"' {
            index += 1;
            continue;
        }

        let start = index + 1;
        index += 1;

        while index < bytes.len() {
            match bytes[index] {
                b'\\' => {
                    index += 2;
                }
                b'"' => break,
                _ => index += 1,
            }
        }

        if index >= bytes.len() {
            break;
        }

        let value = &source[start..index];
        index += 1;

        let mut cursor = index;
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }

        if value == expected && cursor < bytes.len() && bytes[cursor] == b':' {
            last = Some(start - 1);
        }
    }

    last
}

fn find_object_start_after_key(source: &str) -> Option<usize> {
    let bytes = source.as_bytes();

    let closing_quote = find_string_end(bytes, 1)?;
    let mut index = closing_quote + 1;

    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }

    if bytes.get(index) != Some(&b':') {
        return None;
    }

    index += 1;

    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }

    (bytes.get(index) == Some(&b'{')).then_some(index)
}

fn find_string_end(bytes: &[u8], mut index: usize) -> Option<usize> {
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'"' => return Some(index),
            _ => index += 1,
        }
    }

    None
}

fn json_object_depth(source: &str) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut index = 0;
    let mut in_string = false;

    while index < bytes.len() {
        let byte = bytes[index];

        if in_string {
            match byte {
                b'\\' => index += 2,
                b'"' => {
                    in_string = false;
                    index += 1;
                }
                _ => index += 1,
            }

            continue;
        }

        match byte {
            b'"' => {
                in_string = true;
                index += 1;
            }
            b'{' => {
                depth += 1;
                index += 1;
            }
            b'}' => {
                depth = depth.checked_sub(1)?;
                index += 1;
            }
            _ => index += 1,
        }
    }

    (!in_string).then_some(depth)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context<'a>(source: &'a str, value: &'a str) -> ValidationContext<'a> {
        let start = source.find(value).expect("fixture must contain value");
        ValidationContext::new(source, start..start + value.len())
    }

    #[test]
    fn recognizes_auth_inside_docker_auths_registry_entry() {
        let encoded = STANDARD.encode("cribra:CorrectHorseBatteryStaple");
        let source = format!(
            r#"{{
  "auths": {{
    "registry.example.com": {{
      "auth": "{encoded}"
    }}
  }}
}}"#
        );

        assert!(validate_docker_registry(&context(&source, &encoded)).is_some());
    }

    #[test]
    fn rejects_auth_outside_docker_auths_structure() {
        let encoded = STANDARD.encode("cribra:CorrectHorseBatteryStaple");

        for source in [
            format!(r#"{{"auth": "{encoded}"}}"#),
            format!(r#"{{"service": {{"auth": "{encoded}"}}}}"#),
            format!(r#"{{"auths": {{"auth": "{encoded}"}}}}"#),
            format!(
                r#"{{"auths": {{"registry.example.com": {{"nested": {{"auth": "{encoded}"}}}}}}}}"#
            ),
        ] {
            assert!(
                validate_docker_registry(&context(&source, &encoded)).is_none(),
                "unexpected Docker registry validation for {source:?}",
            );
        }
    }

    #[test]
    fn rejects_invalid_and_documentation_credentials() {
        for decoded in [
            "cribra",
            ":CorrectHorseBatteryStaple",
            "cribra:",
            "user:password",
            "username:password",
            "user:passwd",
        ] {
            let encoded = STANDARD.encode(decoded);
            let source =
                format!(r#"{{"auths":{{"registry.example.com":{{"auth":"{encoded}"}}}}}}"#);

            assert!(
                validate_docker_registry(&context(&source, &encoded)).is_none(),
                "unexpected acceptance for {decoded:?}",
            );
        }
    }

    #[test]
    fn recognizes_multiple_sibling_registry_entries_independently() {
        let first = STANDARD.encode("alice:CorrectHorseBatteryStaple");
        let second = STANDARD.encode("bob:AnotherStrongRegistryPassword");
    
        let source = format!(
            r#"{{
      "auths": {{
        "registry-one.example.com": {{
          "auth": "{first}"
        }},
        "registry-two.example.com": {{
          "auth": "{second}"
        }}
      }}
    }}"#
        );
    
        assert!(validate_docker_registry(&context(&source, &first)).is_some());
        assert!(validate_docker_registry(&context(&source, &second)).is_some());
    }

}