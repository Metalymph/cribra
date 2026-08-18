//! Deterministic keyed pseudonymization of detected values.
//!
//! Pseudonymization preserves equality relationships without preserving the
//! original matched value. The same matched value transformed with the same
//! caller-provided key produces the same pseudonym, even across sources.
//!
//! A caller-provided key is mandatory. There is deliberately no default key:
//! a process-global or library-defined key would make unrelated callers share
//! correlation identifiers and would weaken the privacy model.
//!
//! The implementation uses keyed BLAKE3. Matched source values are borrowed
//! only while their replacement is computed and are never stored in reports,
//! findings, options, or transformation metadata.

use crate::ScanReport;

use super::{TransformError, ensure_non_overlapping, validated_spans};

const DEFAULT_PREFIX: &str = "silens_pseudo_";
const DEFAULT_DIGEST_BYTES: usize = 16;
const MIN_DIGEST_BYTES: usize = 8;
const MAX_DIGEST_BYTES: usize = 32;

/// Configuration for deterministic pseudonymization.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PseudonymizationOptions {
    key: [u8; 32],
    prefix: String,
    digest_bytes: usize,
}

impl PseudonymizationOptions {
    /// Creates pseudonymization options with a mandatory 32-byte key.
    ///
    /// The same source value and key produce the same pseudonym. Applications
    /// that want pseudonyms to remain linkable across runs must persist this
    /// key securely. Applications that want unlinkable sessions should provide
    /// a fresh random key for each session.
    #[must_use]
    pub fn new(key: [u8; 32]) -> Self {
        Self {
            key,
            prefix: DEFAULT_PREFIX.to_owned(),
            digest_bytes: DEFAULT_DIGEST_BYTES,
        }
    }

    /// Sets the textual prefix prepended to each pseudonym.
    ///
    /// The prefix is output verbatim and does not affect pseudonym identity.
    #[must_use]
    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// Sets the number of keyed-digest bytes encoded in each pseudonym.
    ///
    /// Values are clamped to `8..=32`. The default is 16 bytes (128 bits),
    /// encoded as 32 lowercase hexadecimal characters.
    #[must_use]
    pub fn digest_bytes(mut self, digest_bytes: usize) -> Self {
        self.digest_bytes = digest_bytes.clamp(MIN_DIGEST_BYTES, MAX_DIGEST_BYTES);
        self
    }

    /// Returns the configured pseudonym prefix.
    #[must_use]
    pub fn prefix_str(&self) -> &str {
        &self.prefix
    }

    /// Returns the number of digest bytes emitted per pseudonym.
    #[must_use]
    pub const fn digest_len_bytes(&self) -> usize {
        self.digest_bytes
    }
}

/// Replaces every independent finding with a deterministic keyed pseudonym.
///
/// Equality is preserved for matched values:
///
/// ```text
/// same value + same key -> same pseudonym
/// ```
///
/// The finding's rule identifier, severity, confidence, source key, and source
/// position do not participate in pseudonym identity.
///
/// # Errors
///
/// Returns [`TransformError`] when a finding span is invalid, is not aligned to
/// UTF-8 boundaries, or overlaps another finding.
///
/// Overlaps are rejected because two findings cannot independently consume the
/// same source bytes while preserving a one-value-to-one-pseudonym mapping.
///
/// # Examples
///
/// ```
/// use silens_scan::{
///     Rule, Scanner, Severity,
///     transform::{PseudonymizationOptions, pseudonymize},
/// };
///
/// let scanner = Scanner::builder()
///     .rule(Rule::literal("secret", "SECRET", Severity::High))
///     .build()?;
///
/// let source = "A=SECRET B=SECRET";
/// let results = scanner.scan([("memory", source)]);
/// let report = results.single_report().expect("one report");
/// let options = PseudonymizationOptions::new([7; 32]);
///
/// let output = pseudonymize(source, report, &options)?;
/// let values = output
///     .split_whitespace()
///     .map(|assignment| assignment.split_once('=').expect("assignment").1)
///     .collect::<Vec<_>>();
///
/// assert_eq!(values.len(), 2);
/// assert_eq!(values[0], values[1]);
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn pseudonymize(
    source: &str,
    report: &ScanReport,
    options: &PseudonymizationOptions,
) -> Result<String, TransformError> {
    let spans = validated_spans(source, report)?;
    ensure_non_overlapping(&spans)?;

    if spans.is_empty() {
        return Ok(source.to_owned());
    }

    let estimated_replacement_len = options
        .prefix
        .len()
        .saturating_add(options.digest_bytes.saturating_mul(2));

    let mut output = String::with_capacity(
        source
            .len()
            .saturating_add(estimated_replacement_len.saturating_mul(spans.len())),
    );
    let mut cursor = 0;

    for span in spans {
        output.push_str(&source[cursor..span.start]);

        let matched = &source[span.start..span.end];
        push_pseudonym(&mut output, matched, options);

        cursor = span.end;
    }

    output.push_str(&source[cursor..]);

    Ok(output)
}

fn push_pseudonym(output: &mut String, matched: &str, options: &PseudonymizationOptions) {
    let digest = blake3::keyed_hash(&options.key, matched.as_bytes());
    let digest = &digest.as_bytes()[..options.digest_bytes];

    output.push_str(&options.prefix);

    for byte in digest {
        use std::fmt::Write as _;
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
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
    fn same_value_and_key_produce_same_pseudonym() {
        let source = "SECRET x SECRET";
        let report = ScanReport::new_with_candidates(
            vec![finding("one", 0, 6), finding("two", 9, 15)],
            Vec::new(),
        );
        let options = PseudonymizationOptions::new([1; 32]);

        let output = pseudonymize(source, &report, &options).unwrap();
        let mut parts = output.split(" x ");

        assert_eq!(parts.next(), parts.next());
    }

    #[test]
    fn different_values_produce_different_pseudonyms() {
        let source = "SECRET x OTHER!";
        let report = ScanReport::new_with_candidates(
            vec![finding("one", 0, 6), finding("two", 9, 15)],
            Vec::new(),
        );
        let options = PseudonymizationOptions::new([2; 32]);

        let output = pseudonymize(source, &report, &options).unwrap();
        let parts = output.split(" x ").collect::<Vec<_>>();

        assert_ne!(parts[0], parts[1]);
    }

    #[test]
    fn different_keys_produce_different_pseudonyms() {
        let source = "SECRET";
        let report = ScanReport::new_with_candidates(vec![finding("secret", 0, 6)], Vec::new());

        let first = pseudonymize(source, &report, &PseudonymizationOptions::new([3; 32])).unwrap();
        let second = pseudonymize(source, &report, &PseudonymizationOptions::new([4; 32])).unwrap();

        assert_ne!(first, second);
    }

    #[test]
    fn rule_identity_does_not_change_pseudonym_identity() {
        let source = "SECRET x SECRET";
        let report = ScanReport::new_with_candidates(
            vec![
                finding("provider-specific", 0, 6),
                finding("generic", 9, 15),
            ],
            Vec::new(),
        );
        let options = PseudonymizationOptions::new([5; 32]);

        let output = pseudonymize(source, &report, &options).unwrap();
        let parts = output.split(" x ").collect::<Vec<_>>();

        assert_eq!(parts[0], parts[1]);
    }

    #[test]
    fn prefix_and_digest_length_are_configurable() {
        let source = "SECRET";
        let report = ScanReport::new_with_candidates(vec![finding("secret", 0, 6)], Vec::new());
        let options = PseudonymizationOptions::new([6; 32])
            .prefix("pseudo:")
            .digest_bytes(8);

        let output = pseudonymize(source, &report, &options).unwrap();

        assert!(output.starts_with("pseudo:"));
        assert_eq!(output.len(), "pseudo:".len() + 16);
        assert_eq!(options.digest_len_bytes(), 8);
    }

    #[test]
    fn digest_length_is_clamped_to_safe_bounds() {
        assert_eq!(
            PseudonymizationOptions::new([0; 32])
                .digest_bytes(1)
                .digest_len_bytes(),
            8,
        );
        assert_eq!(
            PseudonymizationOptions::new([0; 32])
                .digest_bytes(128)
                .digest_len_bytes(),
            32,
        );
    }

    #[test]
    fn empty_report_preserves_source_exactly() {
        let source = "ordinary 😀 UTF-8";
        let options = PseudonymizationOptions::new([7; 32]);

        assert_eq!(
            pseudonymize(source, &ScanReport::default(), &options).unwrap(),
            source,
        );
    }

    #[test]
    fn overlap_is_rejected() {
        let report = ScanReport::new_with_candidates(
            vec![finding("short", 0, 6), finding("long", 0, 10)],
            Vec::new(),
        );
        let options = PseudonymizationOptions::new([8; 32]);

        assert!(matches!(
            pseudonymize("0123456789", &report, &options),
            Err(TransformError::OverlappingSpans { .. }),
        ));
    }

    #[test]
    fn original_value_does_not_survive_transformation() {
        let source = "TOKEN=SUPER_SECRET_VALUE";
        let start = "TOKEN=".len();
        let report = ScanReport::new_with_candidates(
            vec![finding("credential", start, source.len())],
            Vec::new(),
        );
        let options = PseudonymizationOptions::new([9; 32]);

        let output = pseudonymize(source, &report, &options).unwrap();

        assert!(!output.contains("SUPER_SECRET_VALUE"));
        assert!(output.starts_with("TOKEN=silens_pseudo_"));
    }

    #[test]
    fn utf8_values_are_pseudonymized_by_exact_bytes() {
        let source = "PASSWORD=pässwörd😀";
        let start = "PASSWORD=".len();
        let report = ScanReport::new_with_candidates(
            vec![finding("password", start, source.len())],
            Vec::new(),
        );
        let options = PseudonymizationOptions::new([10; 32]);

        let output = pseudonymize(source, &report, &options).unwrap();

        assert!(!output.contains("pässwörd😀"));
        assert!(output.starts_with("PASSWORD=silens_pseudo_"));
    }
}
