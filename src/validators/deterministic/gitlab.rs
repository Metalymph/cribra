//! Structural validation for GitLab authentication tokens.
//!
//! This module validates token shape only. It does not contact GitLab or prove
//! that a token is active.

use crate::validators::utils::{
    has_ascii_len, is_obvious_placeholder, is_opaque_token_byte, non_empty_ascii_with,
};

const MAX_TOKEN_LEN: usize = 255;
const MIN_OPAQUE_PAYLOAD_LEN: usize = 8;

/// GitLab credential family recognized by the structural validator.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum GitLabTokenKind {
    Access,
    OAuthApplicationSecret,
    Deploy,
    RunnerAuthentication,
    RegistrationDerivedRunnerAuthentication,
    CiJob,
    Trigger,
    Feed,
    IncomingMail,
    Agent,
    Workspace,
    Scim,
    FeatureFlagClient,
}

/// Successful GitLab token structural validation.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct GitLabValidation {
    kind: GitLabTokenKind,
}

impl GitLabValidation {
    pub(crate) const fn kind(self) -> GitLabTokenKind {
        self.kind
    }
}

/// Validates the complete structure of a possible GitLab authentication token.
pub(crate) fn validate_gitlab_token(candidate: &str) -> Option<GitLabValidation> {
    if !has_ascii_len(candidate, 1, MAX_TOKEN_LEN) || is_obvious_placeholder(candidate) {
        return None;
    }

    for (prefix, kind) in [
        ("glagent-", GitLabTokenKind::Agent),
        (
            "glrtr-",
            GitLabTokenKind::RegistrationDerivedRunnerAuthentication,
        ),
        ("glsoat-", GitLabTokenKind::Scim),
        ("glffct-", GitLabTokenKind::FeatureFlagClient),
        ("glpat-", GitLabTokenKind::Access),
        ("gloas-", GitLabTokenKind::OAuthApplicationSecret),
        ("gldt-", GitLabTokenKind::Deploy),
        ("glrt-", GitLabTokenKind::RunnerAuthentication),
        ("glcbt-", GitLabTokenKind::CiJob),
        ("glptt-", GitLabTokenKind::Trigger),
        ("glft-", GitLabTokenKind::Feed),
        ("glimt-", GitLabTokenKind::IncomingMail),
        ("glwt-", GitLabTokenKind::Workspace),
    ] {
        if let Some(payload) = candidate.strip_prefix(prefix) {
            return validate_opaque(payload, kind);
        }
    }

    None
}

fn validate_opaque(payload: &str, kind: GitLabTokenKind) -> Option<GitLabValidation> {
    let normalized = payload.to_ascii_lowercase();

    if payload.len() < MIN_OPAQUE_PAYLOAD_LEN
        || is_obvious_placeholder(payload)
        || normalized.contains("token_here")
        || normalized.contains("your_token")
        || normalized.contains("test_token")
        || normalized.contains("placeholder")
        || normalized == "example"
        || !non_empty_ascii_with(payload, is_opaque_token_byte)
    {
        return None;
    }

    Some(GitLabValidation { kind })
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &str = "AbCdEf0123456789_AbCdEf0123456789";

    #[test]
    fn accepts_supported_gitlab_prefixes() {
        let cases = [
            (format!("glpat-{BODY}"), GitLabTokenKind::Access),
            (
                format!("gloas-{BODY}"),
                GitLabTokenKind::OAuthApplicationSecret,
            ),
            (format!("gldt-{BODY}"), GitLabTokenKind::Deploy),
            (
                format!("glrt-{BODY}"),
                GitLabTokenKind::RunnerAuthentication,
            ),
            (
                format!("glrtr-{BODY}"),
                GitLabTokenKind::RegistrationDerivedRunnerAuthentication,
            ),
            (format!("glcbt-{BODY}"), GitLabTokenKind::CiJob),
            (format!("glptt-{BODY}"), GitLabTokenKind::Trigger),
            (format!("glft-{BODY}"), GitLabTokenKind::Feed),
            (format!("glimt-{BODY}"), GitLabTokenKind::IncomingMail),
            (format!("glagent-{BODY}"), GitLabTokenKind::Agent),
            (format!("glwt-{BODY}"), GitLabTokenKind::Workspace),
            (format!("glsoat-{BODY}"), GitLabTokenKind::Scim),
            (format!("glffct-{BODY}"), GitLabTokenKind::FeatureFlagClient),
        ];

        for (candidate, expected) in cases {
            assert_eq!(
                validate_gitlab_token(&candidate).map(GitLabValidation::kind),
                Some(expected),
                "unexpected validation result for {candidate}",
            );
        }
    }

    #[test]
    fn rejects_placeholders_invalid_payloads_and_unknown_prefixes() {
        for candidate in [
            "glpat-your_token_here",
            "glpat-example",
            "glpat-too",
            "glpat-AbCdEf01 invalid",
            "gitlab-AbCdEf0123456789",
            "glxyz-AbCdEf0123456789",
        ] {
            assert!(
                validate_gitlab_token(candidate).is_none(),
                "unexpectedly accepted {candidate}",
            );
        }
    }

    #[test]
    fn does_not_infer_custom_self_managed_prefixes() {
        assert!(validate_gitlab_token("company_pat_AbCdEf0123456789_AbCdEf0123456789").is_none());
    }
}
