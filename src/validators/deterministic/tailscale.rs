//! Structural validation for Tailscale credentials.
//!
//! Tailscale credentials use type-specific prefixes that identify the
//! credential capability. Validation deliberately relies on those authoritative
//! prefixes without assuming an undocumented fixed payload length or alphabet.

use crate::validators::utils::is_obvious_placeholder;

const MAX_TOKEN_LEN: usize = 2048;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum TailscaleCredentialKind {
    ApiAccessToken,
    AuthKey,
    OAuthClientSecret,
    ScimKey,
    WebhookKey,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct TailscaleValidation {
    kind: TailscaleCredentialKind,
}

impl TailscaleValidation {
    pub(crate) const fn kind(self) -> TailscaleCredentialKind {
        self.kind
    }
}

/// Validates a structurally recognizable Tailscale credential.
pub(crate) fn validate_tailscale_credential(candidate: &str) -> Option<TailscaleValidation> {
    if candidate.len() > MAX_TOKEN_LEN
        || !candidate.is_ascii()
        || candidate.chars().any(char::is_control)
        || is_obvious_placeholder(candidate)
    {
        return None;
    }

    let (payload, kind) = if let Some(payload) = candidate.strip_prefix("tskey-api-") {
        (payload, TailscaleCredentialKind::ApiAccessToken)
    } else if let Some(payload) = candidate.strip_prefix("tskey-auth-") {
        (payload, TailscaleCredentialKind::AuthKey)
    } else if let Some(payload) = candidate.strip_prefix("tskey-client-") {
        (payload, TailscaleCredentialKind::OAuthClientSecret)
    } else if let Some(payload) = candidate.strip_prefix("tskey-scim-") {
        (payload, TailscaleCredentialKind::ScimKey)
    } else {
        (
            candidate.strip_prefix("tskey-webhook-")?,
            TailscaleCredentialKind::WebhookKey,
        )
    };

    if is_obvious_placeholder(candidate) || is_obvious_placeholder(payload) {
        return None;
    }

    if payload.is_empty() || payload.chars().any(char::is_whitespace) {
        return None;
    }

    Some(TailscaleValidation { kind })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_supported_credential_families() {
        let cases = [
            (
                "tskey-api-a1B2c3D4e5F6",
                TailscaleCredentialKind::ApiAccessToken,
            ),
            ("tskey-auth-a1B2c3D4e5F6", TailscaleCredentialKind::AuthKey),
            (
                "tskey-client-a1B2c3D4e5F6",
                TailscaleCredentialKind::OAuthClientSecret,
            ),
            ("tskey-scim-a1B2c3D4e5F6", TailscaleCredentialKind::ScimKey),
            (
                "tskey-webhook-a1B2c3D4e5F6",
                TailscaleCredentialKind::WebhookKey,
            ),
        ];

        for (candidate, expected) in cases {
            assert_eq!(
                validate_tailscale_credential(candidate).map(TailscaleValidation::kind),
                Some(expected),
            );
        }
    }

    #[test]
    fn rejects_unknown_and_case_changed_prefixes() {
        for candidate in [
            "tskey-unknown-a1B2c3D4e5F6",
            "TSKEY-api-a1B2c3D4e5F6",
            "tskey-API-a1B2c3D4e5F6",
        ] {
            assert!(validate_tailscale_credential(candidate).is_none());
        }
    }

    #[test]
    fn rejects_missing_or_malformed_payloads() {
        for candidate in [
            "tskey-api-",
            "tskey-auth-",
            "tskey-client-",
            "tskey-scim-",
            "tskey-webhook-",
            "tskey-api-value with spaces",
            "tskey-auth-value\nnext",
        ] {
            assert!(validate_tailscale_credential(candidate).is_none());
        }
    }

    #[test]
    fn rejects_documentation_placeholders() {
        for candidate in [
            "tskey-api-your_token_here",
            "tskey-auth-example_token_here",
            "tskey-client-xxxxxxxx",
        ] {
            assert!(validate_tailscale_credential(candidate).is_none());
        }
    }
}
