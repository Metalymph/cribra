//! Structural validation for Stripe secret material.
//!
//! This module recognizes Stripe secret API keys, restricted API keys and
//! webhook signing secrets by their documented prefixes. It does not contact
//! Stripe or prove that a credential is active.

use crate::validators::utils::{
    has_ascii_len, is_obvious_placeholder, is_opaque_token_byte, non_empty_ascii_with,
};

const MIN_PAYLOAD_LEN: usize = 16;
const MAX_TOKEN_LEN: usize = 255;

/// Stripe credential family recognized by the validator.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum StripeTokenKind {
    /// Live-mode secret API key beginning with `sk_live_`.
    LiveSecretKey,

    /// Test or sandbox secret API key beginning with `sk_test_`.
    TestSecretKey,

    /// Live-mode restricted API key beginning with `rk_live_`.
    LiveRestrictedKey,

    /// Test or sandbox restricted API key beginning with `rk_test_`.
    TestRestrictedKey,

    /// Webhook endpoint signing secret beginning with `whsec_`.
    WebhookSigningSecret,
}

/// Successful Stripe token structural validation.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct StripeValidation {
    kind: StripeTokenKind,
}

impl StripeValidation {
    pub(crate) const fn kind(self) -> StripeTokenKind {
        self.kind
    }
}

/// Validates a possible Stripe secret.
///
/// Publishable keys (`pk_*`) are intentionally excluded because they are not
/// secret credentials.
pub(crate) fn validate_stripe_token(candidate: &str) -> Option<StripeValidation> {
    if !has_ascii_len(candidate, 1, MAX_TOKEN_LEN) || is_obvious_placeholder(candidate) {
        return None;
    }

    let (payload, kind) = if let Some(payload) = candidate.strip_prefix("sk_live_") {
        (payload, StripeTokenKind::LiveSecretKey)
    } else if let Some(payload) = candidate.strip_prefix("sk_test_") {
        (payload, StripeTokenKind::TestSecretKey)
    } else if let Some(payload) = candidate.strip_prefix("rk_live_") {
        (payload, StripeTokenKind::LiveRestrictedKey)
    } else if let Some(payload) = candidate.strip_prefix("rk_test_") {
        (payload, StripeTokenKind::TestRestrictedKey)
    } else {
        let payload = candidate.strip_prefix("whsec_")?;
        (payload, StripeTokenKind::WebhookSigningSecret)
    };

    if payload.len() < MIN_PAYLOAD_LEN
        || is_obvious_placeholder(payload)
        || !non_empty_ascii_with(payload, is_opaque_token_byte)
    {
        return None;
    }

    Some(StripeValidation { kind })
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &str = "AbCdEf0123456789_AbCdEf0123456789";

    #[test]
    fn accepts_documented_secret_prefixes() {
        let cases = [
            (format!("sk_live_{BODY}"), StripeTokenKind::LiveSecretKey),
            (format!("sk_test_{BODY}"), StripeTokenKind::TestSecretKey),
            (
                format!("rk_live_{BODY}"),
                StripeTokenKind::LiveRestrictedKey,
            ),
            (
                format!("rk_test_{BODY}"),
                StripeTokenKind::TestRestrictedKey,
            ),
            (
                format!("whsec_{BODY}"),
                StripeTokenKind::WebhookSigningSecret,
            ),
        ];

        for (candidate, expected) in cases {
            assert_eq!(
                validate_stripe_token(&candidate).map(StripeValidation::kind),
                Some(expected),
            );
        }
    }

    #[test]
    fn rejects_publishable_keys() {
        assert!(validate_stripe_token(&format!("pk_live_{BODY}")).is_none());
        assert!(validate_stripe_token(&format!("pk_test_{BODY}")).is_none());
    }

    #[test]
    fn rejects_short_or_placeholder_payloads() {
        assert!(validate_stripe_token("sk_live_short").is_none());
        assert!(validate_stripe_token("sk_live_your_api_key_here").is_none());
    }

    #[test]
    fn rejects_invalid_alphabet() {
        assert!(validate_stripe_token(&format!("sk_live_{BODY}-bad")).is_none());
    }
}
