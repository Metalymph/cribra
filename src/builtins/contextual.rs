//! Built-in rules whose validation requires nearby source context.
//!
//! These rules use capture-aware patterns: the complete expression discovers
//! an assignment and its key name, while only the named `value` capture is
//! passed to validation and exposed as the final finding span.

use crate::{Remediation, RuleSpec, Severity, validators::dispatch::ValidatorKind};

/// AWS long-term access key ID beginning with `AKIA`.
pub const AWS_ACCESS_KEY_ID: RuleSpec =
    RuleSpec::prefix("aws.access-key-id", "AKIA", Severity::High)
        .with_validator(ValidatorKind::Aws)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// AWS temporary STS access key ID beginning with `ASIA`.
pub const AWS_TEMPORARY_ACCESS_KEY_ID: RuleSpec =
    RuleSpec::prefix("aws.temporary-access-key-id", "ASIA", Severity::High)
        .with_validator(ValidatorKind::Aws)
        .with_remediation(Remediation::RevokeAndRotateCredential);

/// AWS secret access key assigned through a recognized configuration key.
pub const AWS_SECRET_ACCESS_KEY: RuleSpec = RuleSpec::captured_pattern(
    "aws.secret-access-key",
    r#"(?i)["\']?(?:aws_secret_access_key|secret_access_key|aws_secret_key)["\']?\s*[:=]\s*["']?(?P<value>[A-Za-z0-9/+=]{40})"#,
    "value",
    Severity::Critical,
)
.with_validator(ValidatorKind::Aws)
.with_remediation(Remediation::RevokeAndRotateCredential);

/// AWS temporary session token assigned through a recognized configuration key.
pub const AWS_SESSION_TOKEN: RuleSpec = RuleSpec::captured_pattern(
    "aws.session-token",
    r#"(?i)["\']?(?:aws_session_token|session_token|aws_security_token)["\']?\s*[:=]\s*["']?(?P<value>[A-Za-z0-9/_+=-]{80,2048})"#,
    "value",
    Severity::Critical,
)
.with_validator(ValidatorKind::Aws)
.with_remediation(Remediation::RevokeAndRotateCredential);

/// Microsoft Azure or Entra application client secret.
pub const AZURE_CLIENT_SECRET: RuleSpec = RuleSpec::captured_pattern(
    "azure.client-secret",
    r#"(?i)["\']?(?:azure_client_secret|client_secret|clientsecret|client_secret_value|microsoft_provider_authentication_secret)["\']?\s*[:=]\s*["']?(?P<value>[A-Za-z0-9~._+\-/=]{16,255})"#,
    "value",
    Severity::Critical,
)
.with_validator(ValidatorKind::Azure)
.with_remediation(Remediation::RevokeAndRotateCredential);

/// Azure Storage account key.
pub const AZURE_STORAGE_ACCOUNT_KEY: RuleSpec = RuleSpec::captured_pattern(
    "azure.storage-account-key",
    r#"(?i)["\']?(?:account_key|storage_account_key|azure_storage_key)["\']?\s*[:=]\s*["']?(?P<value>[A-Za-z0-9+/=]{40,128})"#,
    "value",
    Severity::Critical,
)
.with_validator(ValidatorKind::Azure)
.with_remediation(Remediation::RevokeAndRotateCredential);

/// Azure shared access signature.
pub const AZURE_SHARED_ACCESS_SIGNATURE: RuleSpec = RuleSpec::captured_pattern(
    "azure.shared-access-signature",
    r#"(?i)["\']?(?:shared_access_signature|sas_token|azure_sas_token)["\']?\s*[:=]\s*["']?(?P<value>[A-Za-z0-9%&=._~+\-/?]{32,2048})"#,
    "value",
    Severity::Critical,
)
.with_validator(ValidatorKind::Azure)
.with_remediation(Remediation::RevokeAndRotateCredential);

/// Google Cloud service-account private-key identifier.
pub const GCP_PRIVATE_KEY_ID: RuleSpec = RuleSpec::captured_pattern(
    "gcp.private-key-id",
    r#"(?i)["']?private_key_id["']?\s*:\s*["'](?P<value>[A-Fa-f0-9]{16,128})"#,
    "value",
    Severity::High,
)
.with_validator(ValidatorKind::Gcp)
.with_remediation(Remediation::RevokeAndRotateCredential);

/// Google Cloud OAuth client secret.
pub const GCP_CLIENT_SECRET: RuleSpec = RuleSpec::captured_pattern(
    "gcp.client-secret",
    r#"(?i)["']?client_secret["']?\s*[:=]\s*["'](?P<value>[^"'\r\n]{16,255})"#,
    "value",
    Severity::Critical,
)
.with_validator(ValidatorKind::Gcp)
.with_remediation(Remediation::RevokeAndRotateCredential);

/// Google Cloud service-account private key in PEM form.
///
/// Escaped JSON `\n` private keys are intentionally deferred until an
/// escape-aware projection mode is implemented.
pub const GCP_PRIVATE_KEY: RuleSpec = RuleSpec::captured_pattern(
    "gcp.private-key",
    r#"(?is)["']?private_key["']?\s*:\s*["'](?P<value>-----BEGIN PRIVATE KEY-----.*?-----END PRIVATE KEY-----)"#,
    "value",
    Severity::Critical,
)
.with_validator(ValidatorKind::Gcp)
.with_remediation(Remediation::ReplacePrivateKey);

/// Generic password field.
pub const PASSWORD_FIELD: RuleSpec = RuleSpec::captured_pattern(
    "generic.password-field",
    r#"(?i)["\']?(?:password|passwd|pwd|admin_password|root_password)["\']?\s*[:=]\s*["']?(?P<value>[^\s"'`;]{8,1024})"#,
    "value",
    Severity::High,
)
.with_validator(ValidatorKind::Password)
.with_remediation(Remediation::RotatePassword);

/// Database password field.
pub const DATABASE_PASSWORD_FIELD: RuleSpec = RuleSpec::captured_pattern(
    "generic.database-password-field",
    r#"(?i)["\']?(?:database_password|db_password|postgres_password|mysql_password|redis_password)["\']?\s*[:=]\s*["']?(?P<value>[^\s"'`;]{8,1024})"#,
    "value",
    Severity::Critical,
)
.with_validator(ValidatorKind::Password)
.with_remediation(Remediation::RotatePassword);

/// Private-key or application passphrase field.
pub const PASSPHRASE_FIELD: RuleSpec = RuleSpec::captured_pattern(
    "generic.passphrase-field",
    r#"(?i)["\']?(?:passphrase|private_key_passphrase)["\']?\s*[:=]\s*["']?(?P<value>[^\s"'`;]{8,1024})"#,
    "value",
    Severity::High,
)
.with_validator(ValidatorKind::Password)
.with_remediation(Remediation::RotatePassword);

/// Hash-like value explicitly associated with sensitive material.
pub const SENSITIVE_HASH: RuleSpec = RuleSpec::captured_pattern(
    "generic.sensitive-hash",
    r#"(?i)["\']?(?:password_hash|passwd_hash|secret_hash|credential_hash|api_key_hash|token_hash)["\']?\s*[:=]\s*["']?(?P<value>[A-Fa-f0-9]{128}|[A-Fa-f0-9]{96}|[A-Fa-f0-9]{64}|[A-Fa-f0-9]{40}|[A-Fa-f0-9]{32})"#,
    "value",
    Severity::Medium,
)
.with_validator(ValidatorKind::SensitiveHash)
.with_remediation(Remediation::ReviewSensitiveHash);

/// Explicit generic API-key field.
pub const GENERIC_API_KEY: RuleSpec = RuleSpec::captured_pattern(
    "generic.api-key",
    r#"(?i)["\']?(?:api_key|apikey|api_token|access_key)["\']?\s*[:=]\s*["']?(?P<value>[^\s"'`;]{16,2048})"#,
    "value",
    Severity::High,
)
.with_validator(ValidatorKind::GenericCredential)
.with_remediation(Remediation::RotateCredential);

/// Explicit generic authentication-token field.
pub const GENERIC_AUTH_TOKEN: RuleSpec = RuleSpec::captured_pattern(
    "generic.auth-token",
    r#"(?i)["\']?(?:token|access_token|auth_token|bearer_token)["\']?\s*[:=]\s*["']?(?P<value>[^\s"'`;]{16,2048})"#,
    "value",
    Severity::High,
)
.with_validator(ValidatorKind::GenericCredential)
.with_remediation(Remediation::RotateCredential);

/// Explicit generic secret field.
pub const GENERIC_SECRET: RuleSpec = RuleSpec::captured_pattern(
    "generic.secret",
    r#"(?i)["']?(?:secret|secret_key|signing_secret|webhook_secret)["']?\s*[:=]\s*["']?(?P<value>[^\s"'`;]{16,2048})"#,
    "value",
    Severity::High,
)
.with_validator(ValidatorKind::GenericCredential)
.with_remediation(Remediation::RemoveSensitiveValue);
