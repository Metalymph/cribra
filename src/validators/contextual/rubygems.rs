//! Contextual validation for RubyGems gem-server API keys.

use super::{
    ValidationContext,
    utils::{DEFAULT_KEY_WINDOW, key_matches, nearest_key},
};

use crate::validators::{
    deterministic::rubygems::validate_rubygems_api_key, utils::is_obvious_placeholder,
};

const MIN_KEY_LEN: usize = 8;
const MAX_KEY_LEN: usize = 2048;

/// Successful RubyGems host API-key validation.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct RubyGemsHostValidation;

/// Validates an API key supplied through `GEM_HOST_API_KEY`.
pub(crate) fn validate_rubygems_host_key(
    context: &ValidationContext<'_>,
) -> Option<RubyGemsHostValidation> {
    let candidate = context.candidate();

    if !(MIN_KEY_LEN..=MAX_KEY_LEN).contains(&candidate.len())
        || candidate.chars().any(char::is_control)
        || is_obvious_placeholder(candidate)
    {
        return None;
    }

    // A structurally identifiable RubyGems.org API key is more specific than
    // the generic GEM_HOST_API_KEY transport surface. Let the deterministic
    // RubyGems rule remain authoritative for that value.
    if validate_rubygems_api_key(candidate).is_some() {
        return None;
    }

    let key = nearest_key(context.before_window(DEFAULT_KEY_WINDOW))?;

    key_matches(key, "gem_host_api_key").then_some(RubyGemsHostValidation)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context<'a>(source: &'a str, value: &'a str) -> ValidationContext<'a> {
        let start = source.find(value).expect("fixture must contain value");
        ValidationContext::new(source, start..start + value.len())
    }

    #[test]
    fn recognizes_host_api_key() {
        let key = "private-gem-host-key-0123456789";
        let source = format!("GEM_HOST_API_KEY={key}");

        assert!(validate_rubygems_host_key(&context(&source, key)).is_some());
    }

    #[test]
    fn rejects_unrelated_api_key_field() {
        let key = "private-gem-host-key-0123456789";
        let source = format!("OTHER_API_KEY={key}");

        assert!(validate_rubygems_host_key(&context(&source, key)).is_none());
    }

    #[test]
    fn rejects_placeholder() {
        let source = "GEM_HOST_API_KEY=your_api_key_here";

        assert!(validate_rubygems_host_key(&context(source, "your_api_key_here")).is_none());
    }

    #[test]
    fn yields_to_structurally_valid_rubygems_api_key() {
        let key = format!("rubygems_{}", "a".repeat(32));
        let source = format!("GEM_HOST_API_KEY={key}");

        assert!(validate_rubygems_host_key(&context(&source, &key)).is_none());
    }
}
