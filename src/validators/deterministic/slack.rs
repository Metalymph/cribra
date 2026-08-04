//! Structural validation for Slack authentication tokens.
//!
//! The validator recognizes documented Slack token prefixes and applies
//! conservative ASCII, length and segment checks. It does not contact Slack or
//! prove that a token is active.

use crate::validators::utils::{has_ascii_len, is_obvious_placeholder};

const MIN_TOKEN_LEN: usize = 24;
const MAX_TOKEN_LEN: usize = 512;

/// Slack credential family recognized by the validator.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum SlackTokenKind {
    /// Bot token beginning with `xoxb-`.
    Bot,

    /// User token beginning with `xoxp-`.
    User,

    /// App-level token beginning with `xapp-`.
    AppLevel,

    /// Workflow token beginning with `xwfp-`.
    Workflow,
}

/// Successful Slack token structural validation.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct SlackValidation {
    kind: SlackTokenKind,
}

impl SlackValidation {
    pub(crate) const fn kind(self) -> SlackTokenKind {
        self.kind
    }
}

/// Validates a possible Slack token.
pub(crate) fn validate_slack_token(candidate: &str) -> Option<SlackValidation> {
    if !has_ascii_len(candidate, MIN_TOKEN_LEN, MAX_TOKEN_LEN) || is_obvious_placeholder(candidate)
    {
        return None;
    }

    let (payload, kind) = if let Some(payload) = candidate.strip_prefix("xoxb-") {
        (payload, SlackTokenKind::Bot)
    } else if let Some(payload) = candidate.strip_prefix("xoxp-") {
        (payload, SlackTokenKind::User)
    } else if let Some(payload) = candidate.strip_prefix("xapp-") {
        (payload, SlackTokenKind::AppLevel)
    } else {
        let payload = candidate.strip_prefix("xwfp-")?;
        (payload, SlackTokenKind::Workflow)
    };

    if is_obvious_placeholder(payload)
        || !payload.bytes().all(is_slack_token_byte)
        || payload.starts_with('-')
        || payload.ends_with('-')
        || payload.split('-').any(str::is_empty)
    {
        return None;
    }

    Some(SlackValidation { kind })
}

#[inline]
const fn is_slack_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAYLOAD: &str = "1234567890-1234567890-AbCdEfGhIjKlMnOpQrStUvWx";

    #[test]
    fn accepts_documented_token_prefixes() {
        let cases = [
            (format!("xoxb-{PAYLOAD}"), SlackTokenKind::Bot),
            (format!("xoxp-{PAYLOAD}"), SlackTokenKind::User),
            (format!("xapp-{PAYLOAD}"), SlackTokenKind::AppLevel),
            (format!("xwfp-{PAYLOAD}"), SlackTokenKind::Workflow),
        ];

        for (candidate, expected) in cases {
            assert_eq!(
                validate_slack_token(&candidate).map(SlackValidation::kind),
                Some(expected),
            );
        }
    }

    #[test]
    fn rejects_unknown_prefix() {
        assert!(validate_slack_token(&format!("xoxa-{PAYLOAD}")).is_none());
    }

    #[test]
    fn rejects_empty_segments_and_invalid_characters() {
        assert!(validate_slack_token("xoxb-1234--abcdefghiABCDEFGHI").is_none());
        assert!(validate_slack_token("xoxb-1234567890-abcdef+invalid").is_none());
    }

    #[test]
    fn rejects_placeholders() {
        assert!(validate_slack_token("xoxb-your_token_here_123456789").is_none());
    }
}
