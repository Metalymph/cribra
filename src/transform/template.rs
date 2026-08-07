//! Share-safe semantic template generation.
//!
//! Templates replace each independent finding with a placeholder derived from
//! its rule identifier. Unlike redaction, template generation preserves the
//! semantic category of each finding while discarding the matched value.
//!
//! Template generation is intentionally strict about overlapping findings:
//! one source span must map to one semantic placeholder. Callers that need a
//! conservative transformation for overlapping findings should use
//! [`redact`](super::redact) instead.

use std::collections::BTreeMap;

use crate::ScanReport;

use super::{TransformError, ensure_non_overlapping, validated_spans};

const DEFAULT_NAMESPACE: &str = "SILENS";

/// Configuration for semantic template placeholders.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TemplateOptions {
    namespace: String,
    numbered: bool,
}

impl Default for TemplateOptions {
    fn default() -> Self {
        Self {
            namespace: DEFAULT_NAMESPACE.to_owned(),
            numbered: false,
        }
    }
}

impl TemplateOptions {
    /// Creates options using the default `SILENS` namespace.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the placeholder namespace.
    ///
    /// Unsafe placeholder characters are normalized when output is produced.
    /// An empty or fully unsupported namespace falls back to `SILENS`.
    #[must_use]
    pub fn namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = namespace.into();
        self
    }

    /// Enables or disables deterministic per-rule occurrence numbers.
    ///
    /// With numbering enabled, repeated findings for the same rule become:
    ///
    /// ```text
    /// <SILENS:secret:1>
    /// <SILENS:secret:2>
    /// ```
    #[must_use]
    pub const fn numbered(mut self, numbered: bool) -> Self {
        self.numbered = numbered;
        self
    }

    /// Returns whether per-rule occurrence numbering is enabled.
    #[must_use]
    pub const fn is_numbered(&self) -> bool {
        self.numbered
    }
}

/// Replaces findings with semantic placeholders using default options.
///
/// A finding produced by rule `stripe.live-secret-key`, for example, becomes:
///
/// ```text
/// <SILENS:stripe.live-secret-key>
/// ```
///
/// Rule identifiers and the namespace are normalized to a conservative ASCII
/// placeholder alphabet. Matched source values never become part of the
/// placeholder.
///
/// # Errors
///
/// Returns [`TransformError`] when a finding span is invalid, is not aligned to
/// UTF-8 boundaries, or overlaps another finding.
///
/// # Examples
///
/// ```
/// use silens_scan::{Rule, Scanner, Severity, transform::template};
///
/// let scanner = Scanner::builder()
///     .rule(Rule::literal("credential", "SECRET", Severity::High))
///     .build()?;
///
/// let source = "TOKEN=SECRET";
/// let results = scanner.scan([("memory", source)]);
/// let report = results.single_report().expect("one report");
///
/// assert_eq!(
///     template(source, report)?,
///     "TOKEN=<SILENS:credential>",
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn template(source: &str, report: &ScanReport) -> Result<String, TransformError> {
    template_with(source, report, &TemplateOptions::default())
}

/// Replaces findings with semantic placeholders using `options`.
///
/// Findings are transformed from left to right after span validation. The
/// output preserves all source text outside detected spans.
///
/// # Errors
///
/// Returns [`TransformError`] when a finding span is invalid, is not aligned to
/// UTF-8 boundaries, or overlaps another finding.
pub fn template_with(
    source: &str,
    report: &ScanReport,
    options: &TemplateOptions,
) -> Result<String, TransformError> {
    let spans = validated_spans(source, report)?;
    ensure_non_overlapping(&spans)?;

    if spans.is_empty() {
        return Ok(source.to_owned());
    }

    let namespace = normalize_component(&options.namespace, DEFAULT_NAMESPACE);
    let mut occurrences = BTreeMap::<&str, usize>::new();
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;

    for span in spans {
        output.push_str(&source[cursor..span.start]);

        let rule_id = span.finding.rule_id().as_str();
        let rule = normalize_component(rule_id, "finding");

        output.push('<');
        output.push_str(&namespace);
        output.push(':');
        output.push_str(&rule);

        if options.numbered {
            let occurrence = occurrences.entry(rule_id).or_default();
            *occurrence += 1;

            output.push(':');
            output.push_str(&occurrence.to_string());
        }

        output.push('>');
        cursor = span.end;
    }

    output.push_str(&source[cursor..]);

    Ok(output)
}

fn normalize_component(value: &str, fallback: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut separator_pending = false;

    for character in value.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
            if separator_pending && !output.is_empty() {
                output.push('-');
            }

            separator_pending = false;
            output.push(character);
        } else {
            separator_pending = true;
        }
    }

    while output.ends_with('-') {
        output.pop();
    }

    if output.is_empty() {
        fallback.to_owned()
    } else {
        output
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
        )
    }

    #[test]
    fn replaces_value_with_rule_placeholder() {
        let report = ScanReport::new(vec![finding("stripe.secret", 6, 12)]);

        assert_eq!(
            template("TOKEN=SECRET", &report).unwrap(),
            "TOKEN=<SILENS:stripe.secret>",
        );
    }

    #[test]
    fn preserves_text_outside_findings() {
        let source = "A=SECRET B=PASSWORD C=public";
        let report = ScanReport::new(vec![finding("api-key", 2, 8), finding("password", 11, 19)]);

        assert_eq!(
            template(source, &report).unwrap(),
            "A=<SILENS:api-key> B=<SILENS:password> C=public",
        );
    }

    #[test]
    fn supports_custom_namespace() {
        let report = ScanReport::new(vec![finding("secret", 6, 12)]);
        let options = TemplateOptions::new().namespace("EXAMPLE");

        assert_eq!(
            template_with("TOKEN=SECRET", &report, &options).unwrap(),
            "TOKEN=<EXAMPLE:secret>",
        );
    }

    #[test]
    fn normalizes_unsafe_placeholder_components() {
        let report = ScanReport::new(vec![finding("custom rule > token", 6, 12)]);
        let options = TemplateOptions::new().namespace("My App!");

        assert_eq!(
            template_with("TOKEN=SECRET", &report, &options).unwrap(),
            "TOKEN=<My-App:custom-rule-token>",
        );
    }

    #[test]
    fn empty_namespace_falls_back_to_silens() {
        let report = ScanReport::new(vec![finding("secret", 6, 12)]);
        let options = TemplateOptions::new().namespace("!!!");

        assert_eq!(
            template_with("TOKEN=SECRET", &report, &options).unwrap(),
            "TOKEN=<SILENS:secret>",
        );
    }

    #[test]
    fn numbering_is_deterministic_per_rule() {
        let source = "SECRET x SECRET y OTHER";
        let report = ScanReport::new(vec![
            finding("secret", 0, 6),
            finding("secret", 9, 15),
            finding("other", 18, 23),
        ]);
        let options = TemplateOptions::new().numbered(true);

        assert_eq!(
            template_with(source, &report, &options).unwrap(),
            "<SILENS:secret:1> x <SILENS:secret:2> y <SILENS:other:1>",
        );
    }

    #[test]
    fn empty_report_preserves_source_exactly() {
        let source = "ordinary 😀 UTF-8";

        assert_eq!(template(source, &ScanReport::default()).unwrap(), source,);
    }

    #[test]
    fn overlapping_findings_are_rejected_as_ambiguous() {
        let report = ScanReport::new(vec![finding("short", 0, 6), finding("long", 0, 10)]);

        assert!(matches!(
            template("0123456789", &report),
            Err(TransformError::OverlappingSpans { .. }),
        ));
    }

    #[test]
    fn adjacent_findings_remain_independent() {
        let report = ScanReport::new(vec![finding("left", 0, 3), finding("right", 3, 6)]);

        assert_eq!(
            template("ABCDEF", &report).unwrap(),
            "<SILENS:left><SILENS:right>",
        );
    }

    #[test]
    fn matched_secret_never_appears_in_placeholder() {
        let source = "TOKEN=SUPER_SECRET_VALUE";
        let start = "TOKEN=".len();
        let report = ScanReport::new(vec![finding("credential", start, source.len())]);

        let output = template(source, &report).unwrap();

        assert_eq!(output, "TOKEN=<SILENS:credential>");
        assert!(!output.contains("SUPER_SECRET_VALUE"));
    }
}
