//! Structural validation for RubyGems.org API keys.
//!
//! Current RubyGems.org API keys use the `rubygems_` prefix. Validation is
//! purely structural and never contacts RubyGems.org or proves key activity.

use crate::validators::utils::is_obvious_placeholder;

const PREFIX: &str = "rubygems_";
const MIN_PAYLOAD_LEN: usize = 32;
const MAX_TOKEN_LEN: usize = 2048;

/// Successful RubyGems API-key validation.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct RubyGemsValidation;

/// Validates a structurally recognizable RubyGems.org API key.
pub(crate) fn validate_rubygems_api_key(candidate: &str) -> Option<RubyGemsValidation> {
    if candidate.len() > MAX_TOKEN_LEN
        || is_obvious_placeholder(candidate)
        || candidate.chars().any(char::is_control)
    {
        return None;
    }

    let payload = candidate.strip_prefix(PREFIX)?;

    if payload.len() < MIN_PAYLOAD_LEN || !payload.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }

    Some(RubyGemsValidation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_structurally_valid_api_keys() {
        for payload_len in [32, 48] {
            let value = format!("rubygems_{}", "a".repeat(payload_len));
            assert!(validate_rubygems_api_key(&value).is_some());
        }
    }

    #[test]
    fn rejects_short_and_malformed_values() {
        for value in [
            "rubygems_short",
            "rubygems_0123456789abcdef0123456789abcde",
            "rubygems_0123456789abcdef0123456789abcdeg",
            "rubygems_0123456789abcdef_123456789abcdef",
            "notrubygems_0123456789abcdef",
        ] {
            assert!(validate_rubygems_api_key(value).is_none());
        }
    }

    #[test]
    fn rejects_documentation_and_identifier_near_misses() {
        for value in [
            "rubygems_your_token_here",
            "rubygems_credentials_path",
            "rubygems_api_key_reference",
        ] {
            assert!(validate_rubygems_api_key(value).is_none());
        }
    }
}
