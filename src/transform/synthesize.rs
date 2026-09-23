//! Deterministic provider-aware synthesis of detected values.
//!
//! Synthesis creates shareable fixture/demo values that preserve useful
//! structural cues without retaining the original matched value.
//!
//! For known built-in rule identifiers, Cribra preserves provider identity
//! and, where practical, the original byte length while deliberately breaking a
//! provider-validating character or structural invariant. For contextual and
//! generic credentials where "validity" is defined mainly by surrounding key
//! context, output is explicitly marked `cribra_synthetic`.
//!
//! Synthesis is deterministic for a given caller key, rule identifier and source
//! span. It does not contact providers and cannot prove global non-existence of
//! an arbitrary credential; instead it produces values that are deliberately
//! synthetic and non-derived from the original secret.

use crate::ScanReport;

use super::{TransformError, ensure_non_overlapping, validated_spans};

const DEFAULT_MARKER: &str = "cribra_synthetic";

/// Configuration for deterministic synthetic-value generation.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SynthesisOptions {
    key: [u8; 32],
    marker: String,
}

impl SynthesisOptions {
    /// Creates deterministic synthesis options with a mandatory 32-byte key.
    ///
    /// Reusing the same key makes fixture generation reproducible. Supplying a
    /// new random key creates a different synthetic dataset.
    #[must_use]
    pub fn new(key: [u8; 32]) -> Self {
        Self {
            key,
            marker: DEFAULT_MARKER.to_owned(),
        }
    }

    /// Sets the marker used by contextual and generic synthetic values.
    ///
    /// Unsupported characters are normalized to `_` before output.
    #[must_use]
    pub fn marker(mut self, marker: impl Into<String>) -> Self {
        self.marker = marker.into();
        self
    }

    /// Returns the configured marker.
    #[must_use]
    pub fn marker_str(&self) -> &str {
        &self.marker
    }
}

/// Replaces each independent finding with a deterministic synthetic value.
///
/// Known built-in rules preserve provider-recognizable structure where that can
/// be done safely. Generic/custom rules fall back to a deterministic
/// marker-based value.
///
/// The generated value never includes bytes from the matched source value.
///
/// # Errors
///
/// Returns [`TransformError`] when a finding span is invalid, is not aligned to
/// UTF-8 boundaries, or overlaps another finding.
pub fn synthesize(
    source: &str,
    report: &ScanReport,
    options: &SynthesisOptions,
) -> Result<String, TransformError> {
    let spans = validated_spans(source, report)?;
    ensure_non_overlapping(&spans)?;

    if spans.is_empty() {
        return Ok(source.to_owned());
    }

    let marker = normalized_marker(&options.marker);
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;

    for span in spans {
        output.push_str(&source[cursor..span.start]);

        let rule_id = span.finding.rule_id().as_str();
        let synthetic = synthetic_value(
            rule_id,
            span.end - span.start,
            span.start,
            span.end,
            &marker,
            &options.key,
        );

        output.push_str(&synthetic);
        cursor = span.end;
    }

    output.push_str(&source[cursor..]);

    Ok(output)
}

fn synthetic_value(
    rule_id: &str,
    original_len: usize,
    start: usize,
    end: usize,
    marker: &str,
    key: &[u8; 32],
) -> String {
    let mut random = SyntheticBytes::new(key, rule_id, start, end);

    builtin_synthetic_value(rule_id, original_len, marker, &mut random)
        .unwrap_or_else(|| contextual_marker(marker, "value", original_len, &mut random))
}

/// Returns the intentional synthesis strategy for a built-in rule.
///
/// `None` is reserved for caller-defined rules. Keeping this lookup separate
/// from the custom fallback makes adding a built-in without synthesis semantics
/// an observable test failure instead of a silent behavior change.
fn builtin_synthetic_value(
    rule_id: &str,
    original_len: usize,
    marker: &str,
    random: &mut SyntheticBytes,
) -> Option<String> {
    match rule_id {
        // GitHub: preserve token family prefix, but force `!` into the opaque
        // body so the output is structurally recognizable and validator-invalid.
        "github.classic-pat" => Some(prefixed_invalid("ghp_", original_len, '!', random)),
        "github.fine-grained-pat" => {
            Some(prefixed_invalid("github_pat_", original_len, '!', random))
        }
        "github.oauth-token" => Some(prefixed_invalid("gho_", original_len, '!', random)),
        "github.app-user-token" => Some(prefixed_invalid("ghu_", original_len, '!', random)),
        "github.app-installation-token" => {
            Some(prefixed_invalid("ghs_", original_len, '!', random))
        }
        "github.app-refresh-token" => Some(prefixed_invalid("ghr_", original_len, '!', random)),
        "github.stateless-installation-token" => Some(fixed_or_padded(
            "ghs_0_SYNTHETIC.invalid.token!",
            original_len,
            random,
        )),

        // GitLab: preserve each documented family prefix while forcing an
        // invalid character into the opaque token body.
        "gitlab.access-token" => Some(prefixed_invalid("glpat-", original_len, '!', random)),
        "gitlab.oauth-application-secret" => {
            Some(prefixed_invalid("gloas-", original_len, '!', random))
        }
        "gitlab.deploy-token" => Some(prefixed_invalid("gldt-", original_len, '!', random)),
        "gitlab.runner-auth-token" => Some(prefixed_invalid("glrt-", original_len, '!', random)),
        "gitlab.registration-derived-runner-auth-token" => {
            Some(prefixed_invalid("glrtr-", original_len, '!', random))
        }
        "gitlab.ci-job-token" => Some(prefixed_invalid("glcbt-", original_len, '!', random)),
        "gitlab.trigger-token" => Some(prefixed_invalid("glptt-", original_len, '!', random)),
        "gitlab.feed-token" => Some(prefixed_invalid("glft-", original_len, '!', random)),
        "gitlab.incoming-mail-token" => Some(prefixed_invalid("glimt-", original_len, '!', random)),
        "gitlab.agent-token" => Some(prefixed_invalid("glagent-", original_len, '!', random)),
        "gitlab.workspace-token" => Some(prefixed_invalid("glwt-", original_len, '!', random)),
        "gitlab.scim-token" => Some(prefixed_invalid("glsoat-", original_len, '!', random)),
        "gitlab.feature-flag-client-token" => {
            Some(prefixed_invalid("glffct-", original_len, '!', random))
        }

        // Stripe.
        "stripe.live-secret-key" => Some(prefixed_invalid("sk_live_", original_len, '!', random)),
        "stripe.test-secret-key" => Some(prefixed_invalid("sk_test_", original_len, '!', random)),
        "stripe.live-restricted-key" => {
            Some(prefixed_invalid("rk_live_", original_len, '!', random))
        }
        "stripe.test-restricted-key" => {
            Some(prefixed_invalid("rk_test_", original_len, '!', random))
        }
        "stripe.webhook-secret" => Some(prefixed_invalid("whsec_", original_len, '!', random)),

        // Cloudflare.
        "cloudflare.global-api-key" => Some(prefixed_invalid("cfk_", original_len, '!', random)),
        "cloudflare.user-api-token" => Some(prefixed_invalid("cfut_", original_len, '!', random)),
        "cloudflare.account-api-token" => {
            Some(prefixed_invalid("cfat_", original_len, '!', random))
        }

        // Slack.
        "slack.bot-token" => Some(prefixed_invalid("xoxb-", original_len, '!', random)),
        "slack.user-token" => Some(prefixed_invalid("xoxp-", original_len, '!', random)),
        "slack.app-level-token" => Some(prefixed_invalid("xapp-", original_len, '!', random)),
        "slack.workflow-token" => Some(prefixed_invalid("xwfp-", original_len, '!', random)),

        // Telegram/JWT preserve the broad visual family while deliberately
        // breaking the scanner-valid alphabet/shape.
        "telegram.bot-token" => Some(fixed_or_padded(
            "00000:CRIBRA_SYNTHETIC_BOT_TOKEN!",
            original_len,
            random,
        )),
        "jwt.signed-compact" => Some(fixed_or_padded(
            "eyS.synthetic.payload.invalid!",
            original_len,
            random,
        )),

        // PEM-like material keeps a readable family marker but does not retain
        // the exact BEGIN/END delimiter accepted by any built-in detector.
        "generic.pkcs8-private-key" => Some(fixed_or_padded(
            "-----BEGIN CRIBRA SYNTHETIC PKCS8 PRIVATE KEY-----\\ninvalid\\n-----END CRIBRA SYNTHETIC PKCS8 PRIVATE KEY-----",
            original_len,
            random,
        )),
        "generic.encrypted-private-key" => Some(fixed_or_padded(
            "-----BEGIN CRIBRA SYNTHETIC ENCRYPTED PRIVATE KEY-----\\ninvalid\\n-----END CRIBRA SYNTHETIC ENCRYPTED PRIVATE KEY-----",
            original_len,
            random,
        )),
        "generic.rsa-private-key" => Some(fixed_or_padded(
            "-----BEGIN CRIBRA SYNTHETIC RSA PRIVATE KEY-----\\ninvalid\\n-----END CRIBRA SYNTHETIC RSA PRIVATE KEY-----",
            original_len,
            random,
        )),
        "generic.ec-private-key" => Some(fixed_or_padded(
            "-----BEGIN CRIBRA SYNTHETIC EC PRIVATE KEY-----\\ninvalid\\n-----END CRIBRA SYNTHETIC EC PRIVATE KEY-----",
            original_len,
            random,
        )),
        "generic.openssh-private-key" => Some(fixed_or_padded(
            "-----BEGIN CRIBRA SYNTHETIC OPENSSH PRIVATE KEY-----\\ninvalid\\n-----END CRIBRA SYNTHETIC OPENSSH PRIVATE KEY-----",
            original_len,
            random,
        )),
        "generic.pgp-private-key" => Some(fixed_or_padded(
            "-----BEGIN CRIBRA SYNTHETIC PGP PRIVATE KEY BLOCK-----\\ninvalid\\n-----END CRIBRA SYNTHETIC PGP PRIVATE KEY BLOCK-----",
            original_len,
            random,
        )),

        // AWS identifiers keep their conventional prefix but violate the
        // uppercase/digit or token alphabet.
        "aws.access-key-id" => Some(prefixed_invalid("AKIA", original_len, 's', random)),
        "aws.temporary-access-key-id" => Some(prefixed_invalid("ASIA", original_len, 's', random)),
        "aws.secret-access-key" => Some(fixed_or_padded(
            "CRIBRA_SYNTHETIC_AWS_SECRET!",
            original_len,
            random,
        )),
        "aws.session-token" => Some(fixed_or_padded(
            "CRIBRA_SYNTHETIC_AWS_SESSION!",
            original_len,
            random,
        )),

        // Azure.
        "azure.client-secret" => Some(contextual_marker(
            marker,
            "azure_client_secret",
            original_len,
            random,
        )),
        "azure.storage-account-key" => Some(fixed_or_padded(
            "CRIBRA_SYNTHETIC_AZURE_STORAGE!",
            original_len,
            random,
        )),
        "azure.shared-access-signature" => Some(fixed_or_padded(
            "CRIBRA_SYNTHETIC_AZURE_SAS!",
            original_len,
            random,
        )),

        // GCP.
        "gcp.private-key-id" => Some(fixed_or_padded(
            "g000000000000000_cribra_synthetic",
            original_len,
            random,
        )),
        "gcp.private-key" => Some(fixed_or_padded(
            "-----BEGIN CRIBRA SYNTHETIC PRIVATE KEY-----\\ninvalid\\n-----END CRIBRA SYNTHETIC PRIVATE KEY-----",
            original_len,
            random,
        )),
        "gcp.escaped-private-key" => Some(fixed_or_padded(
            "-----BEGIN CRIBRA SYNTHETIC PRIVATE KEY-----\\\\ninvalid\\\\n-----END CRIBRA SYNTHETIC PRIVATE KEY-----",
            original_len,
            random,
        )),

        // Contextual encoded credentials use a marker that cannot satisfy the
        // corresponding encoded-value validator (notably Base64 validators).
        "docker.registry-auth" => Some(contextual_marker(
            marker,
            "docker_registry_auth",
            original_len,
            random,
        )),
        "npm.registry-auth-token" => Some(contextual_marker(
            marker,
            "npm_auth_token",
            original_len,
            random,
        )),
        "npm.registry-auth" => Some(contextual_marker(
            marker,
            "npm_legacy_auth",
            original_len,
            random,
        )),
        "npm.registry-password" => Some(contextual_marker(
            marker,
            "npm_password",
            original_len,
            random,
        )),
        "composer.http-basic-password" => Some(contextual_marker(
            marker,
            "composer_http_basic_password",
            original_len,
            random,
        )),
        "composer.bearer-token" => Some(contextual_marker(
            marker,
            "composer_bearer_token",
            original_len,
            random,
        )),
        "composer.bitbucket-consumer-secret" => Some(contextual_marker(
            marker,
            "composer_bitbucket_consumer_secret",
            original_len,
            random,
        )),
        "composer.forgejo-token" => Some(contextual_marker(
            marker,
            "composer_forgejo_token",
            original_len,
            random,
        )),
        "cargo.registry-token" => Some(contextual_marker(
            marker,
            "cargo_registry_token",
            original_len,
            random,
        )),
        "cargo.registry-env-token" => Some(contextual_marker(
            marker,
            "cargo_registry_env_token",
            original_len,
            random,
        )),
        "pypi.repository-token" => Some(contextual_marker(
            marker,
            "pypi_repository_token",
            original_len,
            random,
        )),
        "nuget.package-source-cleartext-password" => {
            Some(prefixed_invalid("", original_len, '\0', random))
        }
        "maven.server-password" => Some(prefixed_invalid("", original_len, '\0', random)),
        "rubygems.api-key" => Some(prefixed_invalid("rubygems_", original_len, '!', random)),
        "rubygems.host-api-key" => Some(contextual_marker(
            marker,
            "rubygems_host_api_key",
            original_len,
            random,
        )),
        "swiftpm.registry-token" => Some(contextual_marker(
            marker,
            "swiftpm_registry_token",
            original_len,
            random,
        )),
        "swiftpm.registry-password" => Some(contextual_marker(
            marker,
            "swiftpm_registry_password",
            original_len,
            random,
        )),
        "swiftpm.source-control-token" => Some(contextual_marker(
            marker,
            "swiftpm_source_control_token",
            original_len,
            random,
        )),
        "swiftpm.netrc-password" => Some(contextual_marker(
            marker,
            "swiftpm_netrc_password",
            original_len,
            random,
        )),
        "gradle.repository-password" => Some(contextual_marker(
            marker,
            "gradle_repository_password",
            original_len,
            random,
        )),
        "gradle.repository-auth-header-value" => Some(contextual_marker(
            marker,
            "gradle_repository_auth_header_value",
            original_len,
            random,
        )),
        "netrc.password" => Some(contextual_marker(
            marker,
            "netrc_password",
            original_len,
            random,
        )),

        // Contextual generic families.
        "generic.password-field" => {
            Some(contextual_marker(marker, "password", original_len, random))
        }
        "generic.database-password-field" => Some(contextual_marker(
            marker,
            "database_password",
            original_len,
            random,
        )),
        "generic.database-connection-password" => Some(contextual_marker(
            marker,
            "database_connection_password",
            original_len,
            random,
        )),
        "generic.passphrase-field" => Some(contextual_marker(
            marker,
            "passphrase",
            original_len,
            random,
        )),
        "generic.sensitive-hash" => Some(fixed_or_padded(
            "g_cribra_synthetic_hash",
            original_len,
            random,
        )),
        "generic.api-key" => Some(contextual_marker(marker, "api_key", original_len, random)),
        "generic.auth-token" => Some(contextual_marker(
            marker,
            "auth_token",
            original_len,
            random,
        )),
        "generic.authorization-bearer" => Some(contextual_marker(
            marker,
            "authorization_bearer",
            original_len,
            random,
        )),
        "generic.authorization-basic" => Some(contextual_marker(
            marker,
            "authorization_basic",
            original_len,
            random,
        )),
        "generic.secret" => Some(contextual_marker(marker, "secret", original_len, random)),
        "mfa.otp-provisioning-secret" => Some(contextual_marker(
            marker,
            "otp_provisioning_secret",
            original_len,
            random,
        )),
        // Tailscale credentials preserve their capability-specific prefix while
        // deliberately violating the opaque credential body.
        "tailscale.api-access-token" => {
            Some(prefixed_invalid("tskey-api-", original_len, '!', random))
        }
        "tailscale.auth-key" => Some(prefixed_invalid("tskey-auth-", original_len, '!', random)),
        "tailscale.oauth-client-secret" => {
            Some(prefixed_invalid("tskey-client-", original_len, '!', random))
        }
        "tailscale.scim-key" => Some(prefixed_invalid("tskey-scim-", original_len, '!', random)),
        "tailscale.webhook-key" => Some(prefixed_invalid(
            "tskey-webhook-",
            original_len,
            '!',
            random,
        )),
        "wireguard.private-key" => Some(contextual_marker(
            marker,
            "wireguard_private_key",
            original_len,
            random,
        )),
        "wireguard.preshared-key" => Some(contextual_marker(
            marker,
            "wireguard_preshared_key",
            original_len,
            random,
        )),
        "system.shadow-password-verifier" => Some(contextual_marker(
            marker,
            "shadow_password_verifier",
            original_len,
            random,
        )),
        "system.htpasswd-password-verifier" => Some(contextual_marker(
            marker,
            "htpasswd_password_verifier",
            original_len,
            random,
        )),
        // NATS NKeys preserve their top-level secret family marker while
        // deliberately violating the Base32 alphabet. Seed subtypes are not
        // recovered from the original secret during synthesis.
        "nats.nkey-seed" => Some(prefixed_invalid("S", original_len, '!', random)),
        "nats.nkey-private-key" => Some(prefixed_invalid("P", original_len, '!', random)),

        _ => None,
    }
}

fn prefixed_invalid(
    prefix: &str,
    total_len: usize,
    invalid: char,
    random: &mut SyntheticBytes,
) -> String {
    if total_len <= prefix.len() {
        return fixed_or_padded("SYNTH", total_len, random);
    }

    let body_len = total_len - prefix.len();
    let mut output = String::with_capacity(total_len);
    output.push_str(prefix);

    if body_len == 1 {
        output.push(invalid);
        return output;
    }

    output.push(invalid);
    push_random_ascii(&mut output, body_len - 1, random);
    output
}

fn contextual_marker(
    marker: &str,
    family: &str,
    total_len: usize,
    random: &mut SyntheticBytes,
) -> String {
    let base = format!("{marker}_{family}_");
    fixed_or_padded(&base, total_len, random)
}

fn fixed_or_padded(base: &str, total_len: usize, random: &mut SyntheticBytes) -> String {
    if total_len == 0 {
        return String::new();
    }

    // If the semantic marker does not fit, do not simply truncate it: that
    // would discard all keyed material and make different synthesis keys emit
    // identical short values. For short outputs, preserving deterministic
    // keyed separation takes precedence over preserving the full marker.
    if base.len() >= total_len {
        let readable_prefix_len = total_len.saturating_sub(4).min(base.len());
        let random_len = total_len - readable_prefix_len;

        let mut output = String::with_capacity(total_len);
        output.push_str(&base[..readable_prefix_len]);
        push_random_ascii(&mut output, random_len, random);

        return output;
    }

    let mut output = String::with_capacity(total_len);
    output.push_str(base);
    push_random_ascii(&mut output, total_len - base.len(), random);
    output
}

fn push_random_ascii(output: &mut String, count: usize, random: &mut SyntheticBytes) {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_";

    for _ in 0..count {
        let index = usize::from(random.next()) % ALPHABET.len();
        output.push(char::from(ALPHABET[index]));
    }
}

fn normalized_marker(marker: &str) -> String {
    let mut normalized = String::with_capacity(marker.len());

    for character in marker.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
            normalized.push(character);
        } else {
            normalized.push('_');
        }
    }

    if normalized.is_empty() {
        DEFAULT_MARKER.to_owned()
    } else {
        normalized
    }
}

struct SyntheticBytes {
    reader: blake3::OutputReader,
    buffer: [u8; 64],
    cursor: usize,
}

impl SyntheticBytes {
    fn new(key: &[u8; 32], rule_id: &str, start: usize, end: usize) -> Self {
        let mut hasher = blake3::Hasher::new_keyed(key);
        hasher.update(b"cribra:synthesis:v1\0");
        hasher.update(rule_id.as_bytes());
        hasher.update(b"\0");
        hasher.update(&(start as u64).to_le_bytes());
        hasher.update(&(end as u64).to_le_bytes());

        Self {
            reader: hasher.finalize_xof(),
            buffer: [0; 64],
            cursor: 64,
        }
    }

    fn next(&mut self) -> u8 {
        if self.cursor == self.buffer.len() {
            self.reader.fill(&mut self.buffer);
            self.cursor = 0;
        }

        let byte = self.buffer[self.cursor];
        self.cursor += 1;
        byte
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Confidence, Finding, Location, RuleId, Severity};

    fn finding(id: &str, start: usize, end: usize) -> Finding {
        Finding::new(
            RuleId::from(id),
            Location::from_span(start, end),
            Severity::High,
            Confidence::High,
            None,
        )
    }

    #[test]
    fn synthesis_is_deterministic_for_key_rule_and_span() {
        let source = "TOKEN=SECRET";
        let report =
            ScanReport::new_with_candidates(vec![finding("custom.secret", 6, 12)], Vec::new());
        let options = SynthesisOptions::new([1; 32]);

        let first = synthesize(source, &report, &options).unwrap();
        let second = synthesize(source, &report, &options).unwrap();

        assert_eq!(first, second);
    }

    #[test]
    fn every_current_builtin_has_explicit_synthesis_semantics() {
        for spec in crate::builtins::CURRENT {
            let mut random = SyntheticBytes::new(&[17; 32], spec.id(), 0, 96);

            assert!(
                builtin_synthetic_value(spec.id(), 96, "cribra_synthetic", &mut random).is_some(),
                "built-in rule `{}` must have explicit synthesis semantics",
                spec.id(),
            );
        }
    }

    #[test]
    fn custom_rules_keep_the_generic_deterministic_fallback() {
        let mut random = SyntheticBytes::new(&[18; 32], "custom.secret", 0, 64);

        let output = synthetic_value("custom.secret", 64, 0, 64, "cribra_synthetic", &[18; 32]);

        assert!(
            builtin_synthetic_value("custom.secret", 64, "cribra_synthetic", &mut random).is_none()
        );
        assert!(output.starts_with("cribra_synthetic_value"));
        assert_eq!(output.len(), 64);
    }

    #[test]
    fn different_keys_produce_different_synthetic_values() {
        let source = "TOKEN=SECRET";
        let report =
            ScanReport::new_with_candidates(vec![finding("custom.secret", 6, 12)], Vec::new());

        let first = synthesize(source, &report, &SynthesisOptions::new([1; 32])).unwrap();
        let second = synthesize(source, &report, &SynthesisOptions::new([2; 32])).unwrap();

        assert_ne!(first, second);
        assert_eq!(first.len(), source.len());
        assert_eq!(second.len(), source.len());
        assert!(!first.contains("SECRET"));
        assert!(!second.contains("SECRET"));
    }

    #[test]
    fn short_values_still_include_keyed_material() {
        let source = "SECRET";
        let report =
            ScanReport::new_with_candidates(vec![finding("custom.secret", 0, 6)], Vec::new());

        let first = synthesize(source, &report, &SynthesisOptions::new([11; 32])).unwrap();
        let second = synthesize(source, &report, &SynthesisOptions::new([12; 32])).unwrap();

        assert_eq!(first.len(), source.len());
        assert_eq!(second.len(), source.len());
        assert_ne!(first, second);
        assert_ne!(first, source);
        assert_ne!(second, source);
    }

    #[test]
    fn stripe_shape_preserves_prefix_and_length_but_breaks_validator_alphabet() {
        let source = "sk_live_1234567890abcdefghijkl";
        let report = ScanReport::new_with_candidates(
            vec![finding("stripe.live-secret-key", 0, source.len())],
            Vec::new(),
        );

        let output = synthesize(source, &report, &SynthesisOptions::new([3; 32])).unwrap();

        assert_eq!(output.len(), source.len());
        assert!(output.starts_with("sk_live_!"));
        assert_ne!(output, source);
    }

    #[test]
    fn github_shape_preserves_family_prefix() {
        let source = "ghp_1234567890abcdefghijklmnop";
        let report = ScanReport::new_with_candidates(
            vec![finding("github.classic-pat", 0, source.len())],
            Vec::new(),
        );

        let output = synthesize(source, &report, &SynthesisOptions::new([4; 32])).unwrap();

        assert_eq!(output.len(), source.len());
        assert!(output.starts_with("ghp_!"));
    }

    #[test]
    fn aws_access_key_preserves_prefix_but_is_not_uppercase_digit_only() {
        let source = "AKIA1234567890ABCDEF";
        let report = ScanReport::new_with_candidates(
            vec![finding("aws.access-key-id", 0, source.len())],
            Vec::new(),
        );

        let output = synthesize(source, &report, &SynthesisOptions::new([5; 32])).unwrap();

        assert_eq!(output.len(), source.len());
        assert!(output.starts_with("AKIAs"));
    }

    #[test]
    fn generic_values_are_explicitly_marked_synthetic() {
        let source = "SUPER_SECRET_VALUE_123456";
        let report = ScanReport::new_with_candidates(
            vec![finding("generic.secret", 0, source.len())],
            Vec::new(),
        );

        let output = synthesize(source, &report, &SynthesisOptions::new([6; 32])).unwrap();

        assert_eq!(output.len(), source.len());
        assert!(output.starts_with("cribra_synthetic_"));
        assert_ne!(output, source);
    }

    #[test]
    fn custom_marker_is_normalized() {
        let source = "SUPER_SECRET_VALUE_123456";
        let report = ScanReport::new_with_candidates(
            vec![finding("generic.secret", 0, source.len())],
            Vec::new(),
        );
        let options = SynthesisOptions::new([7; 32]).marker("MY DEMO!");

        let output = synthesize(source, &report, &options).unwrap();

        assert!(output.starts_with("MY_DEMO__"));
    }

    #[test]
    fn original_secret_bytes_do_not_survive() {
        let source = "TOKEN=SUPER_SECRET_VALUE";
        let start = "TOKEN=".len();
        let report = ScanReport::new_with_candidates(
            vec![finding("generic.secret", start, source.len())],
            Vec::new(),
        );

        let output = synthesize(source, &report, &SynthesisOptions::new([8; 32])).unwrap();

        assert!(!output.contains("SUPER_SECRET_VALUE"));
    }

    #[test]
    fn empty_report_preserves_source() {
        let source = "ordinary 😀 UTF-8";

        assert_eq!(
            synthesize(
                source,
                &ScanReport::default(),
                &SynthesisOptions::new([9; 32]),
            )
            .unwrap(),
            source,
        );
    }

    #[test]
    fn overlaps_are_rejected() {
        let report = ScanReport::new_with_candidates(
            vec![finding("one", 0, 6), finding("two", 0, 10)],
            Vec::new(),
        );

        assert!(matches!(
            synthesize("0123456789", &report, &SynthesisOptions::new([10; 32]),),
            Err(TransformError::OverlappingSpans { .. }),
        ));
    }

    #[test]
    fn synthesis_span_hashing_uses_target_independent_width() {
        let key = [0x53; 32];

        let mut hasher = blake3::Hasher::new_keyed(&key);
        hasher.update(b"cribra:synthesis:v1\0");
        hasher.update(b"demo.secret");
        hasher.update(b"\0");
        hasher.update(&10_u64.to_le_bytes());
        hasher.update(&27_u64.to_le_bytes());

        let mut expected_reader = hasher.finalize_xof();
        let mut expected = [0_u8; 64];
        expected_reader.fill(&mut expected);

        let mut actual = SyntheticBytes::new(&key, "demo.secret", 10, 27);

        for byte in expected {
            assert_eq!(actual.next(), byte);
        }
    }

    #[test]
    fn otp_provisioning_secret_uses_explicit_invalid_contextual_synthesis() {
        let mut random = SyntheticBytes::new(&[19; 32], "mfa.otp-provisioning-secret", 0, 32);

        let output = builtin_synthetic_value(
            "mfa.otp-provisioning-secret",
            32,
            "cribra_synthetic",
            &mut random,
        )
        .expect("OTP provisioning secret must have synthesis semantics");

        assert_eq!(output.len(), 32);
        assert!(output.starts_with("cribra_synthetic"));
        assert!(output.contains('_'));
    }
}
