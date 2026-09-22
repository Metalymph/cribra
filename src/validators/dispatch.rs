//! Static dispatch for candidate validation.

use std::ops::Range;

use crate::{
    Confidence, DetectionMode,
    validators::{
        contextual::{
            ValidationContext,
            aws::{AwsCredentialKind, validate_aws},
            azure::{AzureCredentialKind, validate_azure},
            cargo_registry::validate_cargo_registry,
            composer::{ComposerCredentialKind, validate_composer},
            database_connection::{DatabaseConnectionKind, validate_database_connection},
            docker_registry::validate_docker_registry,
            gcp::{GcpCredentialKind, validate_gcp},
            generic::{GenericCredentialKind, validate_generic_credential},
            gradle::{GradleCredentialKind, validate_gradle},
            hash::{HashKind, validate_sensitive_hash},
            http_basic::validate_http_basic,
            maven::validate_maven,
            netrc::validate_netrc,
            npm_registry::{NpmRegistryCredentialKind, validate_npm_registry},
            nuget::validate_nuget,
            pan::validate_pan,
            password::{PasswordKind, validate_password},
            pypi::validate_pypi,
            rubygems::validate_rubygems_host_key,
            swiftpm::validate_swiftpm,
            swiftpm_netrc::validate_swiftpm_netrc,
            system_password_verifier::{
                SystemPasswordVerifierKind, validate_htpasswd_verifier,
                validate_system_password_verifier,
            },
            wireguard::{WireGuardCredentialKind, validate_wireguard},
        },
        deterministic::{
            cloudflare::{CloudflareTokenKind, validate_cloudflare_token},
            codice_fiscale::validate_codice_fiscale,
            github::{GitHubTokenKind, validate_github_token},
            gitlab::{GitLabTokenKind, validate_gitlab_token},
            iban::validate_iban,
            jwt::{JwtKind, validate_jwt},
            pesel::validate_pesel,
            rubygems::validate_rubygems_api_key,
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
    DockerRegistry,
    NpmRegistry,
    CargoRegistry,
    Pypi,
    Maven,
    ComposerHttpBasicPassword,
    ComposerBearerToken,
    ComposerBitbucketConsumerSecret,
    ComposerForgejoToken,
    RubyGems,
    RubyGemsHost,
    SwiftPm,
    SwiftPmNetrc,
    Gradle,
    CodiceFiscale,
    Pesel,
    GitHub,
    GitLab,
    Stripe,
    Iban,
    Cloudflare,
    Slack,
    Telegram,
    Jwt,
    Aws,
    Azure,
    Gcp,
    DatabaseConnection,
    HttpBasic,
    Pan,
    Password,
    SensitiveHash,
    GenericCredential,
    WireGuard,
    SystemPasswordVerifier,
    Netrc,
    Nuget,
}

impl ValidatorKind {
    /// Returns the public validation mode represented by this internal validator.
    pub(crate) const fn detection_mode(self) -> DetectionMode {
        match self {
            Self::None => DetectionMode::MatcherOnly,
            Self::CodiceFiscale
            | Self::Pesel
            | Self::GitHub
            | Self::GitLab
            | Self::Stripe
            | Self::Iban
            | Self::Cloudflare
            | Self::Slack
            | Self::Telegram
            | Self::Jwt
            | Self::RubyGems => DetectionMode::Deterministic,
            Self::Aws
            | Self::Azure
            | Self::Gcp
            | Self::DockerRegistry
            | Self::NpmRegistry
            | Self::CargoRegistry
            | Self::Pypi
            | Self::Maven
            | Self::ComposerHttpBasicPassword
            | Self::ComposerBearerToken
            | Self::ComposerBitbucketConsumerSecret
            | Self::ComposerForgejoToken
            | Self::RubyGemsHost
            | Self::SwiftPm
            | Self::SwiftPmNetrc
            | Self::Gradle
            | Self::DatabaseConnection
            | Self::SystemPasswordVerifier
            | Self::HttpBasic
            | Self::WireGuard
            | Self::Pan
            | Self::Password
            | Self::SensitiveHash
            | Self::GenericCredential
            | Self::Netrc
            | Self::Nuget => DetectionMode::Contextual,
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum ValidationKind {
    Unvalidated,
    DockerRegistry,
    CargoRegistry,
    NpmRegistry(NpmRegistryCredentialKind),
    Pypi,
    Maven,
    Composer(ComposerCredentialKind),
    RubyGems,
    RubyGemsHost,
    SwiftPm,
    SwiftPmNetrc,
    Gradle(GradleCredentialKind),
    CodiceFiscale,
    Pesel,
    GitHub(GitHubTokenKind),
    GitLab(GitLabTokenKind),
    Stripe(StripeTokenKind),
    Iban,
    Cloudflare(CloudflareTokenKind),
    Slack(SlackTokenKind),
    TelegramBotToken,
    Jwt(JwtKind),
    Aws(AwsCredentialKind),
    Azure(AzureCredentialKind),
    Gcp(GcpCredentialKind),
    DatabaseConnection(DatabaseConnectionKind),
    HttpBasic,
    Pan,
    Password(PasswordKind),
    SensitiveHash(HashKind),
    GenericCredential(GenericCredentialKind),
    WireGuard(WireGuardCredentialKind),
    SystemPasswordVerifier(SystemPasswordVerifierKind),
    Netrc,
    Nuget,
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
        ValidatorKind::NpmRegistry => validate_npm_registry(&context).map(|v| {
            ValidationOutcome::new(ValidationKind::NpmRegistry(v.kind()), Confidence::High)
        }),
        ValidatorKind::CargoRegistry => validate_cargo_registry(&context)
            .map(|_| ValidationOutcome::new(ValidationKind::CargoRegistry, Confidence::High)),
        ValidatorKind::Pypi => validate_pypi(&context)
            .map(|_| ValidationOutcome::new(ValidationKind::Pypi, Confidence::High)),
        ValidatorKind::Maven => validate_maven(&context)
            .map(|_| ValidationOutcome::new(ValidationKind::Maven, Confidence::High)),
        ValidatorKind::ComposerHttpBasicPassword
        | ValidatorKind::ComposerBearerToken
        | ValidatorKind::ComposerBitbucketConsumerSecret
        | ValidatorKind::ComposerForgejoToken => {
            let expected = match validator {
                ValidatorKind::ComposerHttpBasicPassword => {
                    ComposerCredentialKind::HttpBasicPassword
                }
                ValidatorKind::ComposerBearerToken => ComposerCredentialKind::BearerToken,
                ValidatorKind::ComposerBitbucketConsumerSecret => {
                    ComposerCredentialKind::BitbucketConsumerSecret
                }
                ValidatorKind::ComposerForgejoToken => ComposerCredentialKind::ForgejoToken,
                _ => unreachable!(),
            };

            validate_composer(&context, expected).map(|validation| {
                ValidationOutcome::new(
                    ValidationKind::Composer(validation.kind()),
                    Confidence::High,
                )
            })
        }
        ValidatorKind::RubyGems => validate_rubygems_api_key(context.candidate())
            .map(|_| ValidationOutcome::new(ValidationKind::RubyGems, Confidence::High)),
        ValidatorKind::RubyGemsHost => validate_rubygems_host_key(&context)
            .map(|_| ValidationOutcome::new(ValidationKind::RubyGemsHost, Confidence::High)),
        ValidatorKind::SwiftPm => validate_swiftpm(&context)
            .map(|_| ValidationOutcome::new(ValidationKind::SwiftPm, Confidence::High)),
        ValidatorKind::SwiftPmNetrc => validate_swiftpm_netrc(&context)
            .map(|_| ValidationOutcome::new(ValidationKind::SwiftPmNetrc, Confidence::High)),
        ValidatorKind::Gradle => validate_gradle(&context)
            .map(|v| ValidationOutcome::new(ValidationKind::Gradle(v.kind()), Confidence::High)),
        ValidatorKind::DockerRegistry => validate_docker_registry(&context)
            .map(|_| ValidationOutcome::new(ValidationKind::DockerRegistry, Confidence::High)),
        ValidatorKind::CodiceFiscale => validate_codice_fiscale(context.candidate())
            .map(|_| ValidationOutcome::new(ValidationKind::CodiceFiscale, Confidence::High)),
        ValidatorKind::Pesel => validate_pesel(context.candidate())
            .map(|_| ValidationOutcome::new(ValidationKind::Pesel, Confidence::High)),
        ValidatorKind::GitHub => validate_github_token(context.candidate())
            .map(|v| ValidationOutcome::new(ValidationKind::GitHub(v.kind()), Confidence::High)),
        ValidatorKind::GitLab => validate_gitlab_token(context.candidate())
            .map(|v| ValidationOutcome::new(ValidationKind::GitLab(v.kind()), Confidence::High)),
        ValidatorKind::Stripe => validate_stripe_token(context.candidate())
            .map(|v| ValidationOutcome::new(ValidationKind::Stripe(v.kind()), Confidence::High)),
        ValidatorKind::Iban => validate_iban(context.candidate())
            .map(|_| ValidationOutcome::new(ValidationKind::Iban, Confidence::High)),
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
        ValidatorKind::DatabaseConnection => validate_database_connection(&context).map(|v| {
            ValidationOutcome::new(
                ValidationKind::DatabaseConnection(v.kind()),
                Confidence::High,
            )
        }),
        ValidatorKind::HttpBasic => validate_http_basic(&context)
            .map(|_| ValidationOutcome::new(ValidationKind::HttpBasic, Confidence::High)),
        ValidatorKind::WireGuard => validate_wireguard(&context)
            .map(|v| ValidationOutcome::new(ValidationKind::WireGuard(v.kind()), Confidence::High)),
        ValidatorKind::Pan => validate_pan(&context)
            .map(|_| ValidationOutcome::new(ValidationKind::Pan, Confidence::High)),
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
        ValidatorKind::Netrc => validate_netrc(&context)
            .map(|_| ValidationOutcome::new(ValidationKind::Netrc, Confidence::High)),
        ValidatorKind::Nuget => validate_nuget(&context)
            .map(|_| ValidationOutcome::new(ValidationKind::Nuget, Confidence::High)),
        ValidatorKind::SystemPasswordVerifier => validate_system_password_verifier(&context)
            .or_else(|| validate_htpasswd_verifier(&context))
            .map(|validation| {
                ValidationOutcome::new(
                    ValidationKind::SystemPasswordVerifier(validation.kind()),
                    Confidence::High,
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
    fn validator_kinds_expose_their_detection_mode() {
        assert_eq!(
            ValidatorKind::None.detection_mode(),
            DetectionMode::MatcherOnly
        );

        for validator in [
            ValidatorKind::CodiceFiscale,
            ValidatorKind::Pesel,
            ValidatorKind::GitHub,
            ValidatorKind::GitLab,
            ValidatorKind::Stripe,
            ValidatorKind::Iban,
            ValidatorKind::Cloudflare,
            ValidatorKind::Slack,
            ValidatorKind::Telegram,
            ValidatorKind::Jwt,
            ValidatorKind::RubyGems,
        ] {
            assert_eq!(
                validator.detection_mode(),
                DetectionMode::Deterministic,
                "unexpected detection mode for {validator:?}",
            );
        }

        for validator in [
            ValidatorKind::Aws,
            ValidatorKind::Azure,
            ValidatorKind::Gcp,
            ValidatorKind::DatabaseConnection,
            ValidatorKind::HttpBasic,
            ValidatorKind::WireGuard,
            ValidatorKind::DockerRegistry,
            ValidatorKind::NpmRegistry,
            ValidatorKind::CargoRegistry,
            ValidatorKind::Pypi,
            ValidatorKind::Maven,
            ValidatorKind::RubyGemsHost,
            ValidatorKind::SwiftPm,
            ValidatorKind::SwiftPmNetrc,
            ValidatorKind::Gradle,
            ValidatorKind::Pan,
            ValidatorKind::Password,
            ValidatorKind::SensitiveHash,
            ValidatorKind::GenericCredential,
            ValidatorKind::Netrc,
            ValidatorKind::Nuget,
            ValidatorKind::SystemPasswordVerifier,
        ] {
            assert_eq!(
                validator.detection_mode(),
                DetectionMode::Contextual,
                "unexpected detection mode for {validator:?}",
            );
        }
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

    #[test]
    fn dispatches_gitlab_deterministic_validator() {
        let token = "glpat-AbCdEf0123456789_AbCdEf0123456789";
        let source = format!("GITLAB_TOKEN={token}");

        let outcome = validate_candidate(
            ValidatorKind::GitLab,
            &source,
            range_of(&source, token),
            Confidence::Low,
        )
        .expect("GitLab token should validate");

        assert!(matches!(
            outcome.kind(),
            ValidationKind::GitLab(GitLabTokenKind::Access)
        ));
        assert_eq!(outcome.confidence(), Confidence::High);
    }

    #[test]
    fn dispatches_iban_validator() {
        let iban = "IT60X0542811101000000123456";
        let source = format!("iban={iban}");

        let outcome = validate_candidate(
            ValidatorKind::Iban,
            &source,
            range_of(&source, iban),
            Confidence::Low,
        )
        .expect("registry IBAN should validate");

        assert_eq!(outcome.kind(), ValidationKind::Iban);
        assert_eq!(outcome.confidence(), Confidence::High);
    }

    #[test]
    fn dispatches_pan_contextual_validator() {
        let pan = "1234567890123452";
        let source = format!("card_number={pan}");

        let outcome = validate_candidate(
            ValidatorKind::Pan,
            &source,
            range_of(&source, pan),
            Confidence::Low,
        )
        .expect("contextual PAN should validate");

        assert_eq!(outcome.kind(), ValidationKind::Pan);
        assert_eq!(outcome.confidence(), Confidence::High);
    }
}
