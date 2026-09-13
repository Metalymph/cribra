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

/// Azure client secret.
pub const AZURE_CLIENT_SECRET: RuleSpec = RuleSpec::captured_pattern(
    "azure.client-secret",
    r#"(?i)["\']?(?:azure_client_secret|clientsecret|client_secret_value|microsoft_provider_authentication_secret)["\']?\s*[:=]\s*["']?(?P<value>[A-Za-z0-9~._+\-/=]{16,255})"#,
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

/// Google Cloud service-account private key in literal multiline PEM form.
///
/// JSON-escaped service-account keys are handled separately by
/// [`GCP_ESCAPED_PRIVATE_KEY`].
pub const GCP_PRIVATE_KEY: RuleSpec = RuleSpec::captured_pattern(
    "gcp.private-key",
    r#"(?is)["']?private_key["']?\s*:\s*["'](?P<value>-----BEGIN PRIVATE KEY-----\r?\n.*?\r?\n-----END PRIVATE KEY-----)"#,
    "value",
    Severity::Critical,
)
.with_validator(ValidatorKind::Gcp)
.with_remediation(Remediation::ReplacePrivateKey);

/// Google Cloud service-account private key using JSON-escaped newlines.
///
/// The finding span remains the exact escaped representation present in the
/// source. Cribra does not decode or normalize the JSON string.
pub const GCP_ESCAPED_PRIVATE_KEY: RuleSpec = RuleSpec::captured_pattern(
    "gcp.escaped-private-key",
    r#"(?is)["']?private_key["']?\s*:\s*["'](?P<value>-----BEGIN PRIVATE KEY-----\\n.*?\\n-----END PRIVATE KEY-----(?:\\n)?)["']"#,
    "value",
    Severity::Critical,
)
.with_validator(ValidatorKind::Gcp)
.with_remediation(Remediation::ReplacePrivateKey);

// Quoted values may contain internal horizontal whitespace. Projection
// intentionally excludes leading/trailing whitespace and never crosses a
// newline while preserving the exact source bytes inside the detected span.
//
// Quote pairing and multiline quoted-value syntax are not parsed here: Cribra
// detects credentials rather than validating JSON/YAML/TOML/env syntax.
// Malformed surrounding syntax therefore does not suppress an otherwise valid
// contextual credential candidate.

/// Generic password field.
pub const PASSWORD_FIELD: RuleSpec = RuleSpec::captured_pattern(
    "generic.password-field",
    r#"(?i)["']?(?:password|passwd|pwd|admin_password|root_password)["']?\s*[:=]\s*["']?(?P<value>[^\s"'`;](?:[^\r\n"'`;]{0,1022}[^\s"'`;])?)"#,
    "value",
    Severity::High,
)
.with_validator(ValidatorKind::Password)
.with_remediation(Remediation::RotatePassword);

/// Database password field.
pub const DATABASE_PASSWORD_FIELD: RuleSpec = RuleSpec::captured_pattern(
    "generic.database-password-field",
    r#"(?i)["']?(?:database_password|db_password|postgres_password|mysql_password|redis_password)["']?\s*[:=]\s*["']?(?P<value>[^\s"'`;](?:[^\r\n"'`;]{0,1022}[^\s"'`;])?)"#,
    "value",
    Severity::Critical,
)
.with_validator(ValidatorKind::Password)
.with_remediation(Remediation::RotatePassword);

/// Password embedded in a recognized database connection URI.
///
/// Only the password component of `username:password@host` userinfo is exposed
/// as the finding span.
pub const DATABASE_CONNECTION_PASSWORD: RuleSpec = RuleSpec::captured_pattern(
    "generic.database-connection-password",
    r#"(?i)(?:postgres(?:ql)?|mysql|mariadb|mongodb(?:\+srv)?|rediss?)://[^/\s:@]+:(?P<value>[^\s@/?#"'`]{1,1024})@"#,
    "value",
    Severity::Critical,
)
.with_validator(ValidatorKind::DatabaseConnection)
.with_remediation(Remediation::RotatePassword);

/// Private-key or application passphrase field.
pub const PASSPHRASE_FIELD: RuleSpec = RuleSpec::captured_pattern(
    "generic.passphrase-field",
    r#"(?i)["']?(?:passphrase|private_key_passphrase)["']?\s*[:=]\s*["']?(?P<value>[^\s"'`;](?:[^\r\n"'`;]{0,1022}[^\s"'`;])?)"#,
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

/// HTTP Authorization header carrying a Bearer credential.
pub const AUTHORIZATION_BEARER: RuleSpec = RuleSpec::captured_pattern(
    "generic.authorization-bearer",
    r#"(?i)["']?authorization["']?\s*:\s*["']?bearer\s+(?P<value>[^\s"'`;]{16,2048})"#,
    "value",
    Severity::High,
)
.with_validator(ValidatorKind::GenericCredential)
.with_remediation(Remediation::RotateCredential);

/// HTTP Authorization header carrying Basic credentials.
///
/// Only the encoded credential is exposed as the finding span. The payload is
/// decoded locally during validation solely to verify `username:password`
/// structure; generic Base64 data is never scanned.
pub const AUTHORIZATION_BASIC: RuleSpec = RuleSpec::captured_pattern(
    "generic.authorization-basic",
    r#"(?i)["']?authorization["']?\s*:\s*["']?basic\s+(?P<value>[A-Za-z0-9+/=]{4,2048})(?:["'\s;,]|$)"#,
    "value",
    Severity::Critical,
)
.with_validator(ValidatorKind::HttpBasic)
.with_remediation(Remediation::RotatePassword);

/// Explicit generic secret field.
pub const GENERIC_SECRET: RuleSpec = RuleSpec::captured_pattern(
    "generic.secret",
    r#"(?i)["']?(?:secret|secret_key|client_secret|signing_secret|webhook_secret)["']?\s*[:=]\s*["']?(?P<value>[^\s"'`;]{16,2048})"#,
    "value",
    Severity::High,
)
.with_validator(ValidatorKind::GenericCredential)
.with_remediation(Remediation::RemoveSensitiveValue);

/// WireGuard interface private key.
pub const WIREGUARD_PRIVATE_KEY: RuleSpec = RuleSpec::captured_pattern(
    "wireguard.private-key",
    r"(?im)^\s*PrivateKey\s*=\s*(?P<value>[A-Za-z0-9+/]{43}=)\s*(?:#.*)?$",
    "value",
    Severity::Critical,
)
.with_validator(ValidatorKind::WireGuard)
.with_remediation(Remediation::ReplacePrivateKey);

/// WireGuard peer preshared key.
pub const WIREGUARD_PRESHARED_KEY: RuleSpec = RuleSpec::captured_pattern(
    "wireguard.preshared-key",
    r"(?im)^\s*PresharedKey\s*=\s*(?P<value>[A-Za-z0-9+/]{43}=)\s*(?:#.*)?$",
    "value",
    Severity::Critical,
)
.with_validator(ValidatorKind::WireGuard)
.with_remediation(Remediation::ReplacePrivateKey);

/// Docker registry authentication stored in Docker `config.json`.
///
/// Only the encoded `auth` value is projected. The value is decoded locally
/// solely to validate its `username:password` credential structure.
pub const DOCKER_REGISTRY_AUTH: RuleSpec = RuleSpec::captured_pattern(
    "docker.registry-auth",
    r#""auth"\s*:\s*"(?P<value>[A-Za-z0-9+/=]{4,4096})""#,
    "value",
    Severity::Critical,
)
.with_validator(ValidatorKind::DockerRegistry)
.with_remediation(Remediation::RotatePassword);

/// Registry-scoped npm authentication token from `.npmrc`.
pub const NPM_REGISTRY_AUTH_TOKEN: RuleSpec = RuleSpec::captured_pattern(
    "npm.registry-auth-token",
    r"(?im)^[ \t]*//[^/\s:]+(?::[0-9]+)?(?:/[^\r\n:]*)?/:_authToken[ \t]*=[ \t]*(?P<value>[^\s#;]{8,2048})[ \t]*(?:[#;].*)?$",
    "value",
    Severity::Critical,
)
.with_validator(ValidatorKind::NpmRegistry)
.with_remediation(Remediation::RevokeAndRotateCredential);

/// Registry-scoped npm legacy `_auth` credential from `.npmrc`.
pub const NPM_REGISTRY_AUTH: RuleSpec = RuleSpec::captured_pattern(
    "npm.registry-auth",
    r"(?im)^[ \t]*//[^/\s:]+(?::[0-9]+)?(?:/[^\r\n:]*)?/:_auth[ \t]*=[ \t]*(?P<value>[A-Za-z0-9+/=]{4,4096})[ \t]*(?:[#;].*)?$",
    "value",
    Severity::Critical,
)
.with_validator(ValidatorKind::NpmRegistry)
.with_remediation(Remediation::RevokeAndRotateCredential);

/// Registry-scoped npm Base64 password from `.npmrc`.
pub const NPM_REGISTRY_PASSWORD: RuleSpec = RuleSpec::captured_pattern(
    "npm.registry-password",
    r"(?im)^[ \t]*//[^/\s:]+(?::[0-9]+)?(?:/[^\r\n:]*)?/:_password[ \t]*=[ \t]*(?P<value>[A-Za-z0-9+/=]{4,4096})[ \t]*(?:[#;].*)?$",
    "value",
    Severity::Critical,
)
.with_validator(ValidatorKind::NpmRegistry)
.with_remediation(Remediation::RotatePassword);

/// Cargo registry authentication token stored in Cargo credentials/config.
///
/// Contextual validation requires the `token` assignment to belong to either
/// `[registry]` or `[registries.<name>]`.
pub const CARGO_REGISTRY_TOKEN: RuleSpec = RuleSpec::captured_pattern(
    "cargo.registry-token",
    r#"(?m)^[ \t]*token[ \t]*=[ \t]*"(?P<value>[^"\r\n]{1,2048})"[ \t]*(?:#.*)?$"#,
    "value",
    Severity::Critical,
)
.with_validator(ValidatorKind::CargoRegistry)
.with_remediation(Remediation::RevokeAndRotateCredential);

/// Cargo registry authentication token provided through Cargo environment
/// variables.
pub const CARGO_REGISTRY_ENV_TOKEN: RuleSpec = RuleSpec::captured_pattern(
    "cargo.registry-env-token",
    r"(?im)^[ \t]*CARGO_(?:REGISTRY|REGISTRIES_[A-Z0-9_]+)_TOKEN[ \t]*=[ \t]*(?P<value>[^\s#;]{1,2048})[ \t]*(?:[#;].*)?$",
    "value",
    Severity::Critical,
)
.with_validator(ValidatorKind::CargoRegistry)
.with_remediation(Remediation::RevokeAndRotateCredential);

/// PyPI-style repository authentication token stored as the password of a
/// token-authenticated `.pypirc` repository section.
///
/// Contextual validation requires `username = __token__` in the same section
/// and either a standard PyPI section or an explicit repository URL.
pub const PYPI_REPOSITORY_TOKEN: RuleSpec = RuleSpec::captured_pattern(
    "pypi.repository-token",
    r"(?im)^[ \t]*password[ \t]*=[ \t]*(?P<value>[^\s#;]{8,2048})[ \t]*(?:[#;].*)?$",
    "value",
    Severity::Critical,
)
.with_validator(ValidatorKind::Pypi)
.with_remediation(Remediation::RevokeAndRotateCredential);

/// Password belonging to a complete `.netrc` machine credential record.
///
/// `.netrc` is whitespace-oriented, so candidate discovery recognizes an
/// explicit `password <value>` field while contextual validation requires a
/// preceding `machine` and `login` belonging to the same record.
///
/// The matcher deliberately begins at the `password` keyword rather than
/// consuming its leading whitespace. This keeps it compatible with the
/// contextual pattern prefilter, whose match position is used as the regex
/// execution hint.
///
/// Only the password value is exposed as the finding span.
pub const NETRC_PASSWORD: RuleSpec = RuleSpec::captured_pattern(
    "netrc.password",
    r"(?m)\bpassword\b[ \t]+(?P<value>[^\s#]{1,1024})(?:[ \t]*(?:#.*)?$|[ \t]+)",
    "value",
    Severity::Critical,
)
.with_validator(ValidatorKind::Netrc)
.with_remediation(Remediation::RotatePassword);

/// Password verifier stored in the password field of an `/etc/shadow`-style
/// account record.
///
/// Only supported modular-crypt verifier families are candidates. Structural
/// validation requires the verifier to occupy the second field of a complete
/// shadow record.
pub const SHADOW_PASSWORD_VERIFIER: RuleSpec = RuleSpec::captured_pattern(
    "system.shadow-password-verifier",
    r"(?m)^[^:\r\n\s]+:(?P<value>\$(?:5|6|y)\$[^:\r\n]{8,1020}):[^:\r\n]*:[^:\r\n]*:[^:\r\n]*:[^:\r\n]*:[^:\r\n]*:[^:\r\n]*:[^:\r\n]*$",
    "value",
    Severity::High,
)
.with_validator(ValidatorKind::SystemPasswordVerifier)
.with_remediation(Remediation::ReviewPasswordVerifier);

/// Password verifier stored in an Apache `.htpasswd`-style record.
///
/// Only APR1 and bcrypt verifier families are currently supported. The finding
/// projects only the verifier rather than the complete `username:verifier`
/// record.
pub const HTPASSWD_PASSWORD_VERIFIER: RuleSpec = RuleSpec::captured_pattern(
    "system.htpasswd-password-verifier",
    r"(?m)^[^:\r\n\s]+:(?P<value>(?:\$apr1\$[^:\r\n]{1,64}|\$2[aby]\$[^:\r\n]{1,128}))$",
    "value",
    Severity::High,
)
.with_validator(ValidatorKind::SystemPasswordVerifier)
.with_remediation(Remediation::ReviewPasswordVerifier);
