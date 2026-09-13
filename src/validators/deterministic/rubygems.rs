//! Structural validation for RubyGems.org API keys.
//!
//! Current RubyGems.org API keys use the `rubygems_` prefix. Validation is
//! purely structural and never contacts RubyGems.org or proves key activity.

use crate::validators::utils::{is_obvious_placeholder, is_opaque_token_byte};

const PREFIX: &str = "rubygems_";
const MIN_PAYLOAD_LEN: usize = 16;
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

    if payload.len() < MIN_PAYLOAD_LEN || !payload.bytes().all(is_opaque_token_byte) {
        return None;
    }

    Some(RubyGemsValidation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_structurally_valid_api_key() {
        assert!(validate_rubygems_api_key("rubygems_AbCdEfGhIjKlMnOpQrStUvWxYz012345",).is_some());
    }

    #[test]
    fn rejects_short_and_malformed_values() {
        for value in [
            "rubygems_short",
            "rubygems_AbCdEfGhIjKlMnOp+invalid",
            "notrubygems_AbCdEfGhIjKlMnOpQrStUvWx",
        ] {
            assert!(validate_rubygems_api_key(value).is_none());
        }
    }

    #[test]
    fn rejects_documentation_placeholder() {
        assert!(validate_rubygems_api_key("rubygems_your_token_here").is_none());
    }
}
