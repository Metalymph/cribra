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

    match rule_id {
        // GitHub: preserve token family prefix, but force `!` into the opaque
        // body so the output is structurally recognizable and validator-invalid.
        "github.classic-pat" => prefixed_invalid("ghp_", original_len, '!', &mut random),
        "github.fine-grained-pat" => {
            prefixed_invalid("github_pat_", original_len, '!', &mut random)
        }
        "github.oauth-token" => prefixed_invalid("gho_", original_len, '!', &mut random),
        "github.app-user-token" => prefixed_invalid("ghu_", original_len, '!', &mut random),
        "github.app-installation-token" => prefixed_invalid("ghs_", original_len, '!', &mut random),
        "github.app-refresh-token" => prefixed_invalid("ghr_", original_len, '!', &mut random),
        "github.stateless-installation-token" => {
            fixed_or_padded("ghs_0_SYNTHETIC.invalid.token!", original_len, &mut random)
        }

        // Stripe.
        "stripe.live-secret-key" => prefixed_invalid("sk_live_", original_len, '!', &mut random),
        "stripe.test-secret-key" => prefixed_invalid("sk_test_", original_len, '!', &mut random),
        "stripe.live-restricted-key" => {
            prefixed_invalid("rk_live_", original_len, '!', &mut random)
        }
        "stripe.test-restricted-key" => {
            prefixed_invalid("rk_test_", original_len, '!', &mut random)
        }
        "stripe.webhook-secret" => prefixed_invalid("whsec_", original_len, '!', &mut random),

        // Cloudflare.
        "cloudflare.global-api-key" => prefixed_invalid("cfk_", original_len, '!', &mut random),
        "cloudflare.user-api-token" => prefixed_invalid("cfut_", original_len, '!', &mut random),
        "cloudflare.account-api-token" => prefixed_invalid("cfat_", original_len, '!', &mut random),

        // Slack.
        "slack.bot-token" => prefixed_invalid("xoxb-", original_len, '!', &mut random),
        "slack.user-token" => prefixed_invalid("xoxp-", original_len, '!', &mut random),
        "slack.app-level-token" => prefixed_invalid("xapp-", original_len, '!', &mut random),
        "slack.workflow-token" => prefixed_invalid("xwfp-", original_len, '!', &mut random),

        // Telegram/JWT preserve the broad visual family while deliberately
        // breaking the scanner-valid alphabet/shape.
        "telegram.bot-token" => fixed_or_padded(
            "00000:CRIBRA_SYNTHETIC_BOT_TOKEN!",
            original_len,
            &mut random,
        ),
        "jwt.signed-compact" => {
            fixed_or_padded("eyS.synthetic.payload.invalid!", original_len, &mut random)
        }

        // AWS identifiers keep their conventional prefix but violate the
        // uppercase/digit or token alphabet.
        "aws.access-key-id" => prefixed_invalid("AKIA", original_len, 's', &mut random),
        "aws.temporary-access-key-id" => prefixed_invalid("ASIA", original_len, 's', &mut random),
        "aws.secret-access-key" => {
            fixed_or_padded("CRIBRA_SYNTHETIC_AWS_SECRET!", original_len, &mut random)
        }
        "aws.session-token" => {
            fixed_or_padded("CRIBRA_SYNTHETIC_AWS_SESSION!", original_len, &mut random)
        }

        // Azure.
        "azure.client-secret" => {
            contextual_marker(marker, "azure_client_secret", original_len, &mut random)
        }
        "azure.storage-account-key" => {
            fixed_or_padded("CRIBRA_SYNTHETIC_AZURE_STORAGE!", original_len, &mut random)
        }
        "azure.shared-access-signature" => {
            fixed_or_padded("CRIBRA_SYNTHETIC_AZURE_SAS!", original_len, &mut random)
        }

        // GCP.
        "gcp.private-key-id" => fixed_or_padded(
            "g000000000000000_cribra_synthetic",
            original_len,
            &mut random,
        ),
        "gcp.client-secret" => {
            contextual_marker(marker, "gcp_client_secret", original_len, &mut random)
        }
        "gcp.private-key" => fixed_or_padded(
            "-----BEGIN SYNTHETIC PRIVATE KEY-----CRIBRA-----END SYNTHETIC PRIVATE KEY-----",
            original_len,
            &mut random,
        ),

        // Contextual generic families.
        "generic.password-field" => {
            contextual_marker(marker, "password", original_len, &mut random)
        }
        "generic.database-password-field" => {
            contextual_marker(marker, "database_password", original_len, &mut random)
        }
        "generic.passphrase-field" => {
            contextual_marker(marker, "passphrase", original_len, &mut random)
        }
        "generic.sensitive-hash" => {
            fixed_or_padded("g_cribra_synthetic_hash", original_len, &mut random)
        }
        "generic.api-key" => contextual_marker(marker, "api_key", original_len, &mut random),
        "generic.auth-token" => contextual_marker(marker, "auth_token", original_len, &mut random),
        "generic.secret" => contextual_marker(marker, "secret", original_len, &mut random),

        // Custom rules do not imply provider semantics. Keep the value clearly
        // synthetic while making generation deterministic.
        _ => contextual_marker(marker, "value", original_len, &mut random),
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
        hasher.update(&start.to_le_bytes());
        hasher.update(&end.to_le_bytes());

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
}
