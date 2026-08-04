//! Static dispatch for candidate validation.

use std::ops::Range;

use crate::{
    Confidence,
    validators::{
        contextual::{
            ValidationContext,
            aws::{AwsCredentialKind, validate_aws},
            azure::{AzureCredentialKind, validate_azure},
            gcp::{GcpCredentialKind, validate_gcp},
            generic::{GenericCredentialKind, validate_generic_credential},
            hash::{HashKind, validate_sensitive_hash},
            password::{PasswordKind, validate_password},
        },
        deterministic::{
            cloudflare::{CloudflareTokenKind, validate_cloudflare_token},
            github::{GitHubTokenKind, validate_github_token},
            jwt::{JwtKind, validate_jwt},
            slack::{SlackTokenKind, validate_slack_token},
            stripe::{StripeTokenKind, validate_stripe_token},
            telegram::validate_telegram_bot_token,
        },
    },
};

/// The kind of validator to use for a given candidate value.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub(crate) enum ValidatorKind {
    #[default]
    None,
    GitHub,
    Stripe,
    Cloudflare,
    Slack,
    Telegram,
    Jwt,
    Aws,
    Azure,
    Gcp,
    Password,
    SensitiveHash,
    GenericCredential,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum ValidationKind {
    Unvalidated,
    GitHub(GitHubTokenKind),
    Stripe(StripeTokenKind),
    Cloudflare(CloudflareTokenKind),
    Slack(SlackTokenKind),
    TelegramBotToken,
    Jwt(JwtKind),
    Aws(AwsCredentialKind),
    Azure(AzureCredentialKind),
    Gcp(GcpCredentialKind),
    Password(PasswordKind),
    SensitiveHash(HashKind),
    GenericCredential(GenericCredentialKind),
}

/// The outcome of a validation attempt.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct ValidationOutcome {
    kind: ValidationKind,
    confidence: Confidence,
}

impl ValidationOutcome {
    const fn new(kind: ValidationKind, confidence: Confidence) -> Self {
        Self { kind, confidence }
    }

    #[cfg(test)]
    pub(crate) const fn kind(self) -> ValidationKind {
        self.kind
    }

    pub(crate) const fn confidence(self) -> Confidence {
        self.confidence
    }
}

/// Dispatches the appropriate validator for a given candidate value.
/// Validates a candidate value using the appropriate validator based on the provided ValidatorKind.
///
/// The function behaves as follows:
///
/// * If `ValidatorKind::None`, it accepts the match and retains the confidence from the rule.
/// * For deterministic validators, it uses only the candidate substring.
/// * For contextual validators, it constructs a ValidationContext and passes it to the validator.
/// * A None result indicates that the candidate was rejected.
///
/// # Arguments
/// * `validator` - The type of validator to use for validation.
/// * `source` - The full source string containing the candidate.
/// * `candidate` - The range within `source` that represents the candidate value.
/// * `fallback_confidence` - The confidence level to use if no other is available.
///
/// # Returns
/// An `Option<ValidationOutcome>` containing the result of the validation, or `None` if the candidate was rejected.
pub(crate) fn validate_candidate(
    validator: ValidatorKind,
    source: &str,
    candidate: Range<usize>,
    fallback_confidence: Confidence,
) -> Option<ValidationOutcome> {
    debug_assert!(candidate.start <= candidate.end);
    debug_assert!(candidate.end <= source.len());
    debug_assert!(source.is_char_boundary(candidate.start));
    debug_assert!(source.is_char_boundary(candidate.end));

    if validator == ValidatorKind::None {
        return Some(ValidationOutcome::new(
            ValidationKind::Unvalidated,
            fallback_confidence,
        ));
    }

    let context = ValidationContext::new(source, candidate);

    match validator {
        ValidatorKind::None => unreachable!("handled above"),
        ValidatorKind::GitHub => validate_github_token(context.candidate())
            .map(|v| ValidationOutcome::new(ValidationKind::GitHub(v.kind()), Confidence::High)),
        ValidatorKind::Stripe => validate_stripe_token(context.candidate())
            .map(|v| ValidationOutcome::new(ValidationKind::Stripe(v.kind()), Confidence::High)),
        ValidatorKind::Cloudflare => validate_cloudflare_token(context.candidate()).map(|v| {
            ValidationOutcome::new(ValidationKind::Cloudflare(v.kind()), Confidence::High)
        }),
        ValidatorKind::Slack => validate_slack_token(context.candidate())
            .map(|v| ValidationOutcome::new(ValidationKind::Slack(v.kind()), Confidence::High)),
        ValidatorKind::Telegram => validate_telegram_bot_token(context.candidate())
            .map(|_| ValidationOutcome::new(ValidationKind::TelegramBotToken, Confidence::High)),
        ValidatorKind::Jwt => validate_jwt(context.candidate())
            .map(|v| ValidationOutcome::new(ValidationKind::Jwt(v.kind()), Confidence::Medium)),
        ValidatorKind::Aws => validate_aws(&context)
            .map(|v| ValidationOutcome::new(ValidationKind::Aws(v.kind()), Confidence::High)),
        ValidatorKind::Azure => validate_azure(&context)
            .map(|v| ValidationOutcome::new(ValidationKind::Azure(v.kind()), Confidence::High)),
        ValidatorKind::Gcp => validate_gcp(&context)
            .map(|v| ValidationOutcome::new(ValidationKind::Gcp(v.kind()), Confidence::High)),
        ValidatorKind::Password => validate_password(&context).map(|v| {
            ValidationOutcome::new(ValidationKind::Password(v.kind()), Confidence::Medium)
        }),
        ValidatorKind::SensitiveHash => validate_sensitive_hash(&context).map(|v| {
            ValidationOutcome::new(ValidationKind::SensitiveHash(v.kind()), Confidence::Medium)
        }),
        ValidatorKind::GenericCredential => validate_generic_credential(&context).map(|v| {
            ValidationOutcome::new(
                ValidationKind::GenericCredential(v.kind()),
                Confidence::Medium,
            )
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range_of(source: &str, candidate: &str) -> Range<usize> {
        let start = source
            .find(candidate)
            .expect("fixture must contain candidate");
        start..start + candidate.len()
    }

    #[test]
    fn accepts_unvalidated_candidates_with_fallback_confidence() {
        let source = "plain custom match";
        let outcome = validate_candidate(
            ValidatorKind::None,
            source,
            range_of(source, "custom"),
            Confidence::Low,
        )
        .expect("unvalidated candidate should be accepted");

        assert_eq!(outcome.kind(), ValidationKind::Unvalidated);
        assert_eq!(outcome.confidence(), Confidence::Low);
    }

    #[test]
    fn dispatches_deterministic_validator() {
        let token = "ghp_AbCdEf0123456789_AbCdEf0123456789";
        let source = format!("GITHUB_TOKEN={token}");

        let outcome = validate_candidate(
            ValidatorKind::GitHub,
            &source,
            range_of(&source, token),
            Confidence::Low,
        )
        .expect("GitHub token should validate");

        assert!(matches!(
            outcome.kind(),
            ValidationKind::GitHub(GitHubTokenKind::PersonalAccess)
        ));
        assert_eq!(outcome.confidence(), Confidence::High);
    }

    #[test]
    fn dispatches_contextual_validator() {
        let secret = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY";
        let source = format!("AWS_SECRET_ACCESS_KEY={secret}");

        let outcome = validate_candidate(
            ValidatorKind::Aws,
            &source,
            range_of(&source, secret),
            Confidence::Low,
        )
        .expect("AWS secret should validate");

        assert!(matches!(
            outcome.kind(),
            ValidationKind::Aws(AwsCredentialKind::SecretAccessKey)
        ));
    }
}
