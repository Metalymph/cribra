//! Built-in rules for credentials recognizable from their value alone.

use crate::{Remediation, RuleSpec, Severity, validators::dispatch::ValidatorKind};

/// GitHub classic personal access token.
pub const GITHUB_CLASSIC_PAT: RuleSpec =
    RuleSpec::prefix("github.classic-pat", "ghp_", Severity::Critical)
        .with_validator(ValidatorKind::GitHub)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// GitHub fine-grained personal access token.
pub const GITHUB_FINE_GRAINED_PAT: RuleSpec =
    RuleSpec::prefix("github.fine-grained-pat", "github_pat_", Severity::Critical)
        .with_validator(ValidatorKind::GitHub)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// GitHub OAuth access token.
pub const GITHUB_OAUTH_TOKEN: RuleSpec =
    RuleSpec::prefix("github.oauth-token", "gho_", Severity::Critical)
        .with_validator(ValidatorKind::GitHub)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// GitHub App user access token.
pub const GITHUB_APP_USER_TOKEN: RuleSpec =
    RuleSpec::prefix("github.app-user-token", "ghu_", Severity::Critical)
        .with_validator(ValidatorKind::GitHub)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// Opaque GitHub App installation token.
pub const GITHUB_APP_INSTALLATION_TOKEN: RuleSpec =
    RuleSpec::prefix("github.app-installation-token", "ghs_", Severity::Critical)
        .with_validator(ValidatorKind::GitHub)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// Stateless GitHub App installation token.
pub const GITHUB_STATELESS_INSTALLATION_TOKEN: RuleSpec = RuleSpec::pattern(
    "github.stateless-installation-token",
    r"\bghs_[0-9]+_[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b",
    Severity::Critical,
)
.with_validator(ValidatorKind::GitHub)
.with_remediation(Remediation::RevokeAndRotateCredential);

/// GitHub App refresh token.
pub const GITHUB_APP_REFRESH_TOKEN: RuleSpec =
    RuleSpec::prefix("github.app-refresh-token", "ghr_", Severity::Critical)
        .with_validator(ValidatorKind::GitHub)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// GitLab access token.
pub const GITLAB_ACCESS_TOKEN: RuleSpec =
    RuleSpec::prefix("gitlab.access-token", "glpat-", Severity::Critical)
        .with_validator(ValidatorKind::GitLab)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// GitLab deploy token.
pub const GITLAB_DEPLOY_TOKEN: RuleSpec =
    RuleSpec::prefix("gitlab.deploy-token", "gldt-", Severity::Critical)
        .with_validator(ValidatorKind::GitLab)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// GitLab OAuth application secret.
pub const GITLAB_OAUTH_APPLICATION_SECRET: RuleSpec = RuleSpec::prefix(
    "gitlab.oauth-application-secret",
    "gloas-",
    Severity::Critical,
)
.with_validator(ValidatorKind::GitLab)
.with_remediation(Remediation::RevokeAndRotateCredential);

/// GitLab runner authentication token.
pub const GITLAB_RUNNER_AUTH_TOKEN: RuleSpec =
    RuleSpec::prefix("gitlab.runner-auth-token", "glrt-", Severity::Critical)
        .with_validator(ValidatorKind::GitLab)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// GitLab runner authentication token created through a registration token.
pub const GITLAB_REGISTRATION_DERIVED_RUNNER_AUTH_TOKEN: RuleSpec = RuleSpec::prefix(
    "gitlab.registration-derived-runner-auth-token",
    "glrtr-",
    Severity::Critical,
)
.with_validator(ValidatorKind::GitLab)
.with_remediation(Remediation::RevokeAndRotateCredential);

/// GitLab CI/CD job token.
pub const GITLAB_CI_JOB_TOKEN: RuleSpec =
    RuleSpec::prefix("gitlab.ci-job-token", "glcbt-", Severity::Critical)
        .with_validator(ValidatorKind::GitLab)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// GitLab pipeline trigger token.
pub const GITLAB_TRIGGER_TOKEN: RuleSpec =
    RuleSpec::prefix("gitlab.trigger-token", "glptt-", Severity::Critical)
        .with_validator(ValidatorKind::GitLab)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// GitLab feed token.
pub const GITLAB_FEED_TOKEN: RuleSpec =
    RuleSpec::prefix("gitlab.feed-token", "glft-", Severity::Critical)
        .with_validator(ValidatorKind::GitLab)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// GitLab incoming mail token.
pub const GITLAB_INCOMING_MAIL_TOKEN: RuleSpec =
    RuleSpec::prefix("gitlab.incoming-mail-token", "glimt-", Severity::Critical)
        .with_validator(ValidatorKind::GitLab)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// GitLab agent for Kubernetes token.
pub const GITLAB_AGENT_TOKEN: RuleSpec =
    RuleSpec::prefix("gitlab.agent-token", "glagent-", Severity::Critical)
        .with_validator(ValidatorKind::GitLab)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// GitLab workspace token.
pub const GITLAB_WORKSPACE_TOKEN: RuleSpec =
    RuleSpec::prefix("gitlab.workspace-token", "glwt-", Severity::Critical)
        .with_validator(ValidatorKind::GitLab)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// GitLab SCIM access token.
pub const GITLAB_SCIM_TOKEN: RuleSpec =
    RuleSpec::prefix("gitlab.scim-token", "glsoat-", Severity::Critical)
        .with_validator(ValidatorKind::GitLab)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// GitLab feature flag client token.
pub const GITLAB_FEATURE_FLAG_CLIENT_TOKEN: RuleSpec = RuleSpec::prefix(
    "gitlab.feature-flag-client-token",
    "glffct-",
    Severity::Critical,
)
.with_validator(ValidatorKind::GitLab)
.with_remediation(Remediation::RevokeAndRotateCredential);

/// Stripe live-mode secret API key.
pub const STRIPE_LIVE_SECRET_KEY: RuleSpec =
    RuleSpec::prefix("stripe.live-secret-key", "sk_live_", Severity::Critical)
        .with_validator(ValidatorKind::Stripe)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// Stripe test-mode secret API key.
pub const STRIPE_TEST_SECRET_KEY: RuleSpec =
    RuleSpec::prefix("stripe.test-secret-key", "sk_test_", Severity::High)
        .with_validator(ValidatorKind::Stripe)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// Stripe live-mode restricted API key.
pub const STRIPE_LIVE_RESTRICTED_KEY: RuleSpec =
    RuleSpec::prefix("stripe.live-restricted-key", "rk_live_", Severity::Critical)
        .with_validator(ValidatorKind::Stripe)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// Stripe test-mode restricted API key.
pub const STRIPE_TEST_RESTRICTED_KEY: RuleSpec =
    RuleSpec::prefix("stripe.test-restricted-key", "rk_test_", Severity::High)
        .with_validator(ValidatorKind::Stripe)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// Stripe webhook endpoint signing secret.
pub const STRIPE_WEBHOOK_SIGNING_SECRET: RuleSpec =
    RuleSpec::prefix("stripe.webhook-secret", "whsec_", Severity::Critical)
        .with_validator(ValidatorKind::Stripe)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// Cloudflare global API key.
pub const CLOUDFLARE_GLOBAL_API_KEY: RuleSpec =
    RuleSpec::prefix("cloudflare.global-api-key", "cfk_", Severity::Critical)
        .with_validator(ValidatorKind::Cloudflare)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// Cloudflare user-scoped API token.
pub const CLOUDFLARE_USER_API_TOKEN: RuleSpec =
    RuleSpec::prefix("cloudflare.user-api-token", "cfut_", Severity::Critical)
        .with_validator(ValidatorKind::Cloudflare)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// Cloudflare account-owned API token.
pub const CLOUDFLARE_ACCOUNT_API_TOKEN: RuleSpec =
    RuleSpec::prefix("cloudflare.account-api-token", "cfat_", Severity::Critical)
        .with_validator(ValidatorKind::Cloudflare)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// Slack bot token.
pub const SLACK_BOT_TOKEN: RuleSpec =
    RuleSpec::prefix("slack.bot-token", "xoxb-", Severity::Critical)
        .with_validator(ValidatorKind::Slack)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// Slack user token.
pub const SLACK_USER_TOKEN: RuleSpec =
    RuleSpec::prefix("slack.user-token", "xoxp-", Severity::Critical)
        .with_validator(ValidatorKind::Slack)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// Slack app-level token.
pub const SLACK_APP_LEVEL_TOKEN: RuleSpec =
    RuleSpec::prefix("slack.app-level-token", "xapp-", Severity::Critical)
        .with_validator(ValidatorKind::Slack)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// Slack workflow token.
pub const SLACK_WORKFLOW_TOKEN: RuleSpec =
    RuleSpec::prefix("slack.workflow-token", "xwfp-", Severity::Critical)
        .with_validator(ValidatorKind::Slack)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// Telegram Bot API token.
pub const TELEGRAM_BOT_TOKEN: RuleSpec = RuleSpec::pattern(
    "telegram.bot-token",
    r"\b[0-9]{5,20}:[A-Za-z0-9_-]{20,128}\b",
    Severity::Critical,
)
.with_validator(ValidatorKind::Telegram)
.with_remediation(Remediation::RevokeAndRotateCredential);

/// Signed JWT/JWS compact serialization.
pub const SIGNED_JWT: RuleSpec = RuleSpec::pattern(
    "jwt.signed-compact",
    r"\beyJ[A-Za-z0-9_-]*\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b",
    Severity::High,
)
.with_validator(ValidatorKind::Jwt)
.with_remediation(Remediation::RemoveSensitiveValue);

/// Generic PKCS#8 private key in PEM form.
pub const PKCS8_PRIVATE_KEY: RuleSpec = RuleSpec::pattern(
    "generic.pkcs8-private-key",
    r"(?s)-----BEGIN PRIVATE KEY-----\r?\n.{16,}?-----END PRIVATE KEY-----",
    Severity::Critical,
)
.with_remediation(Remediation::ReplacePrivateKey);

/// Generic encrypted PKCS#8 private key in PEM form.
pub const ENCRYPTED_PRIVATE_KEY: RuleSpec = RuleSpec::pattern(
    "generic.encrypted-private-key",
    r"(?s)-----BEGIN ENCRYPTED PRIVATE KEY-----\r?\n.{16,}?-----END ENCRYPTED PRIVATE KEY-----",
    Severity::Critical,
)
.with_remediation(Remediation::ReplacePrivateKey);

/// Generic PKCS#1 RSA private key in PEM form.
pub const RSA_PRIVATE_KEY: RuleSpec = RuleSpec::pattern(
    "generic.rsa-private-key",
    r"(?s)-----BEGIN RSA PRIVATE KEY-----\r?\n.{16,}?-----END RSA PRIVATE KEY-----",
    Severity::Critical,
)
.with_remediation(Remediation::ReplacePrivateKey);

/// Generic SEC1 EC private key in PEM form.
pub const EC_PRIVATE_KEY: RuleSpec = RuleSpec::pattern(
    "generic.ec-private-key",
    r"(?s)-----BEGIN EC PRIVATE KEY-----\r?\n.{16,}?-----END EC PRIVATE KEY-----",
    Severity::Critical,
)
.with_remediation(Remediation::ReplacePrivateKey);

/// Generic OpenSSH private key in PEM form.
pub const OPENSSH_PRIVATE_KEY: RuleSpec = RuleSpec::pattern(
    "generic.openssh-private-key",
    r"(?s)-----BEGIN OPENSSH PRIVATE KEY-----\r?\n.{16,}?-----END OPENSSH PRIVATE KEY-----",
    Severity::Critical,
)
.with_remediation(Remediation::ReplacePrivateKey);

/// ASCII-armored OpenPGP private-key block.
pub const PGP_PRIVATE_KEY: RuleSpec = RuleSpec::pattern(
    "generic.pgp-private-key",
    r"(?s)-----BEGIN PGP PRIVATE KEY BLOCK-----\r?\n.{16,}?\r?\n-----END PGP PRIVATE KEY BLOCK-----",
    Severity::Critical,
)
.with_remediation(Remediation::ReplacePrivateKey);
