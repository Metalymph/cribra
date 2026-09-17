//! Contextual validation for Composer credentials.
//!
//! Composer authentication configuration can contain repository credentials in
//! JSON objects such as `http-basic`. Validation is intentionally structural:
//! a generic `password` field is not enough to establish Composer ownership.

use super::context::ValidationContext;
use crate::validators::utils::is_obvious_placeholder;

const MAX_CREDENTIAL_LEN: usize = 2048;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub(crate) enum ComposerCredentialKind {
    HttpBasicPassword,
    BearerToken,
    BitbucketConsumerSecret,
    ForgejoToken,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct ComposerValidation {
    kind: ComposerCredentialKind,
}

impl ComposerValidation {
    pub(crate) const fn kind(self) -> ComposerCredentialKind {
        self.kind
    }
}

pub(crate) fn validate_composer(
    context: &ValidationContext<'_>,
    expected: ComposerCredentialKind,
) -> Option<ComposerValidation> {
    let candidate = context.candidate();

    if candidate.is_empty()
        || candidate.len() > MAX_CREDENTIAL_LEN
        || candidate.chars().any(char::is_control)
        || is_obvious_placeholder(candidate)
        || is_composer_documentation_value(candidate)
    {
        return None;
    }

    let valid = match expected {
        ComposerCredentialKind::HttpBasicPassword => is_http_basic_password(context),
        ComposerCredentialKind::BearerToken => is_bearer_token(context),
        ComposerCredentialKind::BitbucketConsumerSecret => is_bitbucket_consumer_secret(context),
        ComposerCredentialKind::ForgejoToken => is_forgejo_token(context),
    };

    valid.then_some(ComposerValidation { kind: expected })
}

fn is_composer_documentation_value(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "password"
            | "your_password"
            | "your_password_here"
            | "example_password"
            | "composer_password"
            | "replace_me"
            | "replace_me_here"
    )
}

fn is_http_basic_password(context: &ValidationContext<'_>) -> bool {
    is_nested_repository_property(context, "http-basic", "password")
}

fn is_bearer_token(context: &ValidationContext<'_>) -> bool {
    const CONTEXT_WINDOW: usize = 16 * 1024;

    let before = context.before_window(CONTEXT_WINDOW);
    let candidate = context.candidate();
    let after = context.after_window(CONTEXT_WINDOW);

    let mut source = String::with_capacity(before.len() + candidate.len() + after.len());
    source.push_str(before);
    source.push_str(candidate);
    source.push_str(after);

    let target = before.len();

    let Some(path) = json_property_path_at(&source, target) else {
        return false;
    };

    path.len() >= 2 && path[path.len() - 2] == "bearer" && !path[path.len() - 1].is_empty()
}

fn is_nested_repository_property(
    context: &ValidationContext<'_>,
    section: &str,
    property: &str,
) -> bool {
    const CONTEXT_WINDOW: usize = 16 * 1024;

    let before = context.before_window(CONTEXT_WINDOW);
    let candidate = context.candidate();
    let after = context.after_window(CONTEXT_WINDOW);

    let mut source = String::with_capacity(before.len() + candidate.len() + after.len());
    source.push_str(before);
    source.push_str(candidate);
    source.push_str(after);

    let Some(path) = json_property_path_at(&source, before.len()) else {
        return false;
    };

    path.len() >= 3
        && path[path.len() - 3] == section
        && !path[path.len() - 2].is_empty()
        && path[path.len() - 1] == property
}

fn is_bitbucket_consumer_secret(context: &ValidationContext<'_>) -> bool {
    is_nested_repository_property(context, "bitbucket-oauth", "consumer-secret")
}

fn is_forgejo_token(context: &ValidationContext<'_>) -> bool {
    is_nested_repository_property(context, "forgejo-token", "token")
}

fn json_property_path_at(source: &str, target: usize) -> Option<Vec<String>> {
    if target > source.len() || !source.is_char_boundary(target) {
        return None;
    }

    let bytes = source.as_bytes();
    let mut cursor = 0;
    let mut stack: Vec<Option<String>> = Vec::new();
    let mut expecting_value_for: Option<String> = None;

    while cursor < target {
        skip_whitespace(bytes, &mut cursor);

        if cursor >= target {
            break;
        }

        match bytes[cursor] {
            b'"' => {
                let (value, next) = parse_json_string(source, cursor)?;

                if next > target {
                    return expecting_value_for.map(|key| {
                        let mut path = stack
                            .iter()
                            .filter_map(|entry| entry.clone())
                            .collect::<Vec<_>>();
                        path.push(key);
                        path
                    });
                }

                cursor = next;
                skip_whitespace(bytes, &mut cursor);

                if cursor < bytes.len() && bytes[cursor] == b':' {
                    cursor += 1;
                    expecting_value_for = Some(value);
                } else if expecting_value_for.is_some() {
                    expecting_value_for = None;
                }
            }
            b'{' => {
                stack.push(expecting_value_for.take());
                cursor += 1;
            }
            b'}' => {
                stack.pop()?;
                expecting_value_for = None;
                cursor += 1;
            }
            b'[' => {
                stack.push(expecting_value_for.take());
                cursor += 1;
            }
            b']' => {
                stack.pop()?;
                expecting_value_for = None;
                cursor += 1;
            }
            b',' => {
                expecting_value_for = None;
                cursor += 1;
            }
            b':' => {
                cursor += 1;
            }
            _ => {
                if expecting_value_for.is_some() {
                    let start = cursor;

                    while cursor < bytes.len() && !matches!(bytes[cursor], b',' | b'}' | b']') {
                        cursor += 1;
                    }

                    if target >= start && target < cursor {
                        let key = expecting_value_for.take()?;
                        let mut path = stack
                            .iter()
                            .filter_map(|entry| entry.clone())
                            .collect::<Vec<_>>();
                        path.push(key);
                        return Some(path);
                    }

                    expecting_value_for = None;
                } else {
                    cursor += 1;
                }
            }
        }
    }

    expecting_value_for.map(|key| {
        let mut path = stack
            .iter()
            .filter_map(|entry| entry.clone())
            .collect::<Vec<_>>();
        path.push(key);
        path
    })
}

fn parse_json_string(source: &str, start: usize) -> Option<(String, usize)> {
    let bytes = source.as_bytes();

    if bytes.get(start) != Some(&b'"') {
        return None;
    }

    let mut cursor = start + 1;
    let mut value = String::new();

    while cursor < bytes.len() {
        match bytes[cursor] {
            b'"' => return Some((value, cursor + 1)),
            b'\\' => {
                cursor += 1;
                let escaped = *bytes.get(cursor)?;

                match escaped {
                    b'"' => value.push('"'),
                    b'\\' => value.push('\\'),
                    b'/' => value.push('/'),
                    b'b' => value.push('\u{0008}'),
                    b'f' => value.push('\u{000c}'),
                    b'n' => value.push('\n'),
                    b'r' => value.push('\r'),
                    b't' => value.push('\t'),
                    b'u' => {
                        let end = cursor.checked_add(5)?;
                        let digits = source.get(cursor + 1..end)?;
                        let code = u16::from_str_radix(digits, 16).ok()?;
                        let character = char::from_u32(u32::from(code))?;
                        value.push(character);
                        cursor = end - 1;
                    }
                    _ => return None,
                }

                cursor += 1;
            }
            _ => {
                let remaining = source.get(cursor..)?;
                let character = remaining.chars().next()?;
                value.push(character);
                cursor += character.len_utf8();
            }
        }
    }

    None
}

fn skip_whitespace(source: &[u8], cursor: &mut usize) {
    while source
        .get(*cursor)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        *cursor += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context<'a>(source: &'a str, value: &'a str) -> ValidationContext<'a> {
        let start = source.find(value).expect("fixture must contain value");
        ValidationContext::new(source, start..start + value.len())
    }

    fn kind(
        source: &str,
        value: &str,
        expected: ComposerCredentialKind,
    ) -> Option<ComposerCredentialKind> {
        validate_composer(&context(source, value), expected).map(ComposerValidation::kind)
    }

    #[test]
    fn recognizes_http_basic_password() {
        let password = "ComposerSecret_123456";
        let source = format!(
            r#"{{
  "http-basic": {{
    "repo.example": {{
      "username": "alice",
      "password": "{password}"
    }}
  }}
}}"#
        );

        assert_eq!(
            kind(&source, password, ComposerCredentialKind::HttpBasicPassword),
            Some(ComposerCredentialKind::HttpBasicPassword),
        );
    }

    #[test]
    fn rejects_generic_password_object() {
        let password = "ComposerSecret_123456";
        let source = format!(r#"{{"password":"{password}"}}"#);

        assert_eq!(
            kind(&source, password, ComposerCredentialKind::HttpBasicPassword),
            None
        );
    }

    #[test]
    fn rejects_unrelated_password_section() {
        let password = "ComposerSecret_123456";
        let source = format!(
            r#"{{
  "application": {{
    "password": "{password}"
  }}
}}"#
        );

        assert_eq!(
            kind(&source, password, ComposerCredentialKind::HttpBasicPassword),
            None
        );
    }

    #[test]
    fn does_not_cross_sibling_object_boundaries() {
        let password = "ComposerSecret_123456";
        let source = format!(
            r#"{{
  "http-basic": {{
    "repo.example": {{
      "username": "alice"
    }}
  }},
  "application": {{
    "password": "{password}"
  }}
}}"#
        );

        assert_eq!(
            kind(&source, password, ComposerCredentialKind::HttpBasicPassword),
            None
        );
    }

    #[test]
    fn handles_json_structural_characters_inside_strings() {
        let password = "ComposerSecret_123456";
        let source = format!(
            r#"{{
  "note": "ignored }} {{ \" text",
  "http-basic": {{
    "repo.example": {{
      "username": "alice",
      "password": "{password}"
    }}
  }}
}}"#
        );

        assert_eq!(
            kind(&source, password, ComposerCredentialKind::HttpBasicPassword),
            Some(ComposerCredentialKind::HttpBasicPassword),
        );
    }

    #[test]
    fn rejects_placeholders() {
        for password in [
            "password",
            "your_password",
            "your_password_here",
            "example_password",
            "composer_password",
            "replace_me",
        ] {
            let source = format!(
                r#"{{
  "http-basic": {{
    "repo.example": {{
      "username": "alice",
      "password": "{password}"
    }}
  }}
}}"#
            );

            assert_eq!(
                kind(&source, password, ComposerCredentialKind::HttpBasicPassword,),
                None,
                "placeholder unexpectedly validated",
            );
        }
    }

    #[test]
    fn rejects_empty_repository_identity() {
        let password = "ComposerSecret_123456";
        let source = format!(
            r#"{{
  "http-basic": {{
    "": {{
      "username": "alice",
      "password": "{password}"
    }}
  }}
}}"#
        );

        assert_eq!(
            kind(&source, password, ComposerCredentialKind::HttpBasicPassword,),
            None,
        );
    }

    #[test]
    fn recognizes_bearer_token() {
        let token = "ComposerBearerToken_123456";
        let source = format!(
            r#"{{
      "bearer": {{
        "repo.example": "{token}"
      }}
    }}"#
        );

        assert_eq!(
            kind(&source, token, ComposerCredentialKind::BearerToken),
            Some(ComposerCredentialKind::BearerToken),
        );
    }

    #[test]
    fn rejects_unattributed_bearer_like_token() {
        let token = "ComposerBearerToken_123456";

        for source in [
            format!(r#"{{"repo.example":"{token}"}}"#),
            format!(r#"{{"application":{{"repo.example":"{token}"}}}}"#),
            format!(r#"{{"bearer":{{"":"{token}"}}}}"#),
        ] {
            assert_eq!(
                kind(&source, token, ComposerCredentialKind::BearerToken),
                None,
            );
        }
    }

    #[test]
    fn bearer_does_not_cross_sibling_object_boundaries() {
        let token = "ComposerBearerToken_123456";
        let source = format!(
            r#"{{
      "bearer": {{
        "repo.example": "other-token"
      }},
      "application": {{
        "repo.example": "{token}"
      }}
    }}"#
        );

        assert_eq!(
            kind(&source, token, ComposerCredentialKind::BearerToken),
            None,
        );
    }

    #[test]
    fn recognizes_bitbucket_consumer_secret() {
        let secret = "BitbucketConsumerSecret_123456";
        let source = format!(
            r#"{{
      "bitbucket-oauth": {{
        "bitbucket.org": {{
          "consumer-key": "consumer-key",
          "consumer-secret": "{secret}"
        }}
      }}
    }}"#
        );

        assert_eq!(
            kind(
                &source,
                secret,
                ComposerCredentialKind::BitbucketConsumerSecret,
            ),
            Some(ComposerCredentialKind::BitbucketConsumerSecret),
        );
    }

    #[test]
    fn recognizes_forgejo_token() {
        let token = "ForgejoAccessToken_123456";
        let source = format!(
            r#"{{
      "forgejo-token": {{
        "forgejo.example.org": {{
          "username": "alice",
          "token": "{token}"
        }}
      }}
    }}"#
        );

        assert_eq!(
            kind(&source, token, ComposerCredentialKind::ForgejoToken),
            Some(ComposerCredentialKind::ForgejoToken),
        );
    }

    #[test]
    fn rejects_unattributed_provider_credentials() {
        let secret = "ProviderSecret_123456";

        for source in [
            format!(r#"{{"application":{{"consumer-secret":"{secret}"}}}}"#),
            format!(r#"{{"bitbucket-oauth":{{"":{{"consumer-secret":"{secret}"}}}}}}"#),
        ] {
            assert_eq!(
                kind(
                    &source,
                    secret,
                    ComposerCredentialKind::BitbucketConsumerSecret,
                ),
                None,
            );
        }

        for source in [
            format!(r#"{{"application":{{"token":"{secret}"}}}}"#),
            format!(r#"{{"forgejo-token":{{"":{{"token":"{secret}"}}}}}}"#),
        ] {
            assert_eq!(
                kind(&source, secret, ComposerCredentialKind::ForgejoToken),
                None,
            );
        }
    }

    #[test]
    fn bearer_does_not_accept_http_basic_password() {
        let password = "ComposerRepositorySecret_123456";
        let source = format!(
            r#"{{"http-basic":{{"repo.example":{{"username":"alice","password":"{password}"}}}}}}"#
        );

        assert!(
            validate_composer(
                &context(&source, password),
                ComposerCredentialKind::BearerToken,
            )
            .is_none()
        );
    }

    #[test]
    fn http_basic_does_not_accept_bearer_token() {
        let token = "ComposerBearerToken_123456";
        let source = format!(r#"{{"bearer":{{"repo.example":"{token}"}}}}"#);

        assert!(
            validate_composer(
                &context(&source, token),
                ComposerCredentialKind::HttpBasicPassword,
            )
            .is_none()
        );
    }
}
