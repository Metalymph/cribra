//! Structural validation for GitHub authentication tokens.
//!
//! This module validates token shape only. It does not contact GitHub or prove
//! that a token is active.

use crate::validators::utils::{
    has_ascii_len, is_ascii_decimal, is_base64url_segment, is_obvious_placeholder,
    is_opaque_token_byte, non_empty_ascii_with,
};

const MAX_TOKEN_LEN: usize = 255;
const MIN_OPAQUE_PAYLOAD_LEN: usize = 20;

/// GitHub credential family recognized by the structural validator.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum GitHubTokenKind {
    PersonalAccess,
    FineGrainedPersonalAccess,
    OAuthAccess,
    AppUserAccess,
    AppInstallationAccess,
    StatelessAppInstallationAccess,
    AppRefresh,
}

/// Successful GitHub token structural validation.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct GitHubValidation {
    kind: GitHubTokenKind,
}

impl GitHubValidation {
    pub(crate) const fn kind(self) -> GitHubTokenKind {
        self.kind
    }
}

/// Validates the complete structure of a possible GitHub authentication token.
pub(crate) fn validate_github_token(candidate: &str) -> Option<GitHubValidation> {
    if !has_ascii_len(candidate, 1, MAX_TOKEN_LEN) || is_obvious_placeholder(candidate) {
        return None;
    }

    if let Some(payload) = candidate.strip_prefix("github_pat_") {
        return validate_opaque(payload, GitHubTokenKind::FineGrainedPersonalAccess);
    }

    if let Some(payload) = candidate.strip_prefix("ghp_") {
        return validate_opaque(payload, GitHubTokenKind::PersonalAccess);
    }

    if let Some(payload) = candidate.strip_prefix("gho_") {
        return validate_opaque(payload, GitHubTokenKind::OAuthAccess);
    }

    if let Some(payload) = candidate.strip_prefix("ghu_") {
        return validate_opaque(payload, GitHubTokenKind::AppUserAccess);
    }

    if let Some(payload) = candidate.strip_prefix("ghr_") {
        return validate_opaque(payload, GitHubTokenKind::AppRefresh);
    }

    if let Some(payload) = candidate.strip_prefix("ghs_") {
        if validate_stateless_installation(payload) {
            return Some(GitHubValidation {
                kind: GitHubTokenKind::StatelessAppInstallationAccess,
            });
        }

        return validate_opaque(payload, GitHubTokenKind::AppInstallationAccess);
    }

    None
}

fn validate_opaque(payload: &str, kind: GitHubTokenKind) -> Option<GitHubValidation> {
    if payload.len() < MIN_OPAQUE_PAYLOAD_LEN
        || is_obvious_placeholder(payload)
        || !non_empty_ascii_with(payload, is_opaque_token_byte)
    {
        return None;
    }

    Some(GitHubValidation { kind })
}

fn validate_stateless_installation(payload: &str) -> bool {
    let Some((app_id, jwt)) = payload.split_once('_') else {
        return false;
    };

    if !is_ascii_decimal(app_id) {
        return false;
    }

    let mut segments = jwt.split('.');
    let Some(header) = segments.next() else {
        return false;
    };
    let Some(claims) = segments.next() else {
        return false;
    };
    let Some(signature) = segments.next() else {
        return false;
    };

    segments.next().is_none()
        && [header, claims, signature]
            .into_iter()
            .all(is_base64url_segment)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &str = "AbCdEf0123456789_AbCdEf0123456789";

    #[test]
    fn accepts_documented_opaque_prefixes() {
        let cases = [
            (format!("ghp_{BODY}"), GitHubTokenKind::PersonalAccess),
            (
                format!("github_pat_{BODY}"),
                GitHubTokenKind::FineGrainedPersonalAccess,
            ),
            (format!("gho_{BODY}"), GitHubTokenKind::OAuthAccess),
            (format!("ghu_{BODY}"), GitHubTokenKind::AppUserAccess),
            (
                format!("ghs_{BODY}"),
                GitHubTokenKind::AppInstallationAccess,
            ),
            (format!("ghr_{BODY}"), GitHubTokenKind::AppRefresh),
        ];

        for (candidate, expected) in cases {
            assert_eq!(
                validate_github_token(&candidate).map(GitHubValidation::kind),
                Some(expected),
            );
        }
    }

    #[test]
    fn accepts_stateless_installation_shape() {
        let candidate = "ghs_123456_eyJhbGciOiJSUzI1NiJ9.eyJpc3MiOiIxMjM0NTYifQ.signature_123";

        assert_eq!(
            validate_github_token(candidate).map(GitHubValidation::kind),
            Some(GitHubTokenKind::StatelessAppInstallationAccess),
        );
    }

    #[test]
    fn rejects_placeholders_and_invalid_shapes() {
        assert!(validate_github_token("ghp_your_token_here").is_none());
        assert!(validate_github_token("ghp_too_short").is_none());
        assert!(validate_github_token(&format!("ghp_{BODY}-invalid")).is_none());
        assert!(validate_github_token("ghs_app_header.claims.signature").is_none());
    }
}
