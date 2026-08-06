use std::sync::Arc;

use crate::{
    compiled_rule::{CompiledRuleSet, RuleMetadata},
    finding::Finding,
    location::Location,
    report::ScanReport,
    scan_entry::ScanEntry,
    scan_results::ScanResults,
    scanner_builder::ScannerBuilder,
    validators::dispatch::validate_candidate,
};

/// Immutable scanner that executes a precompiled set of detection rules.
///
/// A scanner performs no rule validation or matcher compilation while
/// scanning. All configuration work is completed by [`ScannerBuilder::build`],
/// allowing the same scanner instance to be reused across multiple UTF-8
/// inputs.
///
/// Scanning is currently deliberately single-threaded. The execution engine is
/// optimized and benchmarked in serial before any optional parallel strategy
/// is introduced.
#[derive(Debug, Clone)]
pub struct Scanner {
    rules: Arc<CompiledRuleSet>,
}

/// Validated internal candidate awaiting deterministic normalization.
///
/// This stage owns no rule metadata and no source text. It simply ties an
/// accepted byte span to immutable compiled metadata and the confidence
/// produced by validation.
#[derive(Debug, Copy, Clone)]
struct AcceptedCandidate<'a> {
    metadata: &'a RuleMetadata,
    start: usize,
    end: usize,
    confidence: crate::Confidence,
}

impl AcceptedCandidate<'_> {
    const fn same_span(self, other: Self) -> bool {
        self.start == other.start && self.end == other.end
    }
}

/// Internal candidate counts used only by scanner diagnostics tests.
///
/// This type is intentionally unavailable to library consumers and contributes
/// no branches, counters or synchronization to the production scan path.
#[cfg(test)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct ScanDiagnostics {
    raw_candidates: usize,
    accepted_candidates: usize,
    normalized_candidates: usize,
    findings: usize,
}

#[cfg(test)]
impl ScanDiagnostics {
    const fn rejected_candidates(self) -> usize {
        self.raw_candidates - self.accepted_candidates
    }

    const fn collapsed_candidates(self) -> usize {
        self.accepted_candidates - self.normalized_candidates
    }
}

impl Scanner {
    pub(crate) fn new(rules: Arc<CompiledRuleSet>) -> Self {
        Self { rules }
    }

    /// Creates an empty builder for configuring a scanner.
    #[must_use]
    pub const fn builder() -> ScannerBuilder {
        ScannerBuilder::new()
    }

    /// Returns the number of rules compiled into this scanner.
    #[must_use]
    pub fn rules_count(&self) -> usize {
        self.rules.len()
    }

    /// Returns `true` when this scanner contains no rules.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Scans a UTF-8 string and returns a deterministic report.
    ///
    /// The execution pipeline is:
    ///
    /// 1. execute every compiled matcher group into one candidate buffer;
    /// 2. dispatch only the validator selected by each rule;
    /// 3. reject invalid candidates;
    /// 4. normalize accepted candidates deterministically and collapse exact
    ///    duplicate spans;
    /// 5. resolve one-based line and Unicode-scalar column coordinates in one
    ///    forward pass;
    /// 6. materialize the accepted public [`Finding`] values.
    ///
    /// `start` and `end` locations remain zero-based byte offsets. `line` and
    /// `column` are one-based, and columns count Unicode scalar values rather
    /// than UTF-8 bytes.
    /// Scans identified UTF-8 sources in input order.
    ///
    /// Every input is a `(key, source)` tuple. The key is preserved unchanged
    /// in the returned [`ScanResults`] and is never interpreted by the scanner.
    ///
    /// A single source is represented by a one-element collection:
    ///
    /// ```
    /// # use silens_scan::Scanner;
    /// let scanner = Scanner::default();
    /// let results = scanner.scan([("memory", "ordinary text")]);
    ///
    /// assert_eq!(results.len(), 1);
    /// ```
    ///
    /// The per-source execution pipeline is:
    ///
    /// 1. execute every compiled matcher group into one candidate buffer;
    /// 2. dispatch only the validator selected by each rule;
    /// 3. reject invalid candidates;
    /// 4. normalize accepted candidates deterministically;
    /// 5. resolve one-based line and Unicode-scalar column coordinates;
    /// 6. materialize an immutable [`ScanReport`].
    #[must_use]
    pub fn scan<'a, K, I>(&self, inputs: I) -> ScanResults<K>
    where
        I: IntoIterator<Item = (K, &'a str)>,
    {
        ScanResults::new(
            inputs
                .into_iter()
                .map(|(key, source)| ScanEntry::new(key, source.len(), self.scan_source(source)))
                .collect(),
        )
    }

    /// Scans one UTF-8 source through the compiled pipeline.
    fn scan_source(&self, source: &str) -> ScanReport {
        let mut raw_candidates = Vec::new();
        self.rules.scan(source, &mut raw_candidates);

        let mut accepted = Vec::with_capacity(raw_candidates.len());

        for candidate in raw_candidates {
            let metadata = self.rules.metadata(candidate.rule_index());

            let Some(validation) = validate_candidate(
                metadata.validator(),
                source,
                candidate.start()..candidate.end(),
                metadata.confidence(),
            ) else {
                continue;
            };

            accepted.push(AcceptedCandidate {
                metadata,
                start: candidate.start(),
                end: candidate.end(),
                confidence: validation.confidence(),
            });
        }

        normalize_candidates(&mut accepted);

        let mut findings = Vec::with_capacity(accepted.len());
        let mut cursor = 0;
        let mut line = 1;
        let mut column = 1;

        for candidate in accepted {
            advance_position(source, &mut cursor, candidate.start, &mut line, &mut column);

            let mut location = Location::from_span(candidate.start, candidate.end);
            location.set_position(line, column);

            findings.push(Finding::new(
                candidate.metadata.id().clone(),
                location,
                candidate.metadata.severity(),
                candidate.confidence,
            ));
        }

        ScanReport::new(findings)
    }

    /// Executes the candidate stages and returns internal counts for tests.
    ///
    /// This deliberately duplicates the small orchestration portion of
    /// the production source pipeline under `cfg(test)` so the production path remains free
    /// from diagnostic branches and counters.
    #[cfg(test)]
    fn diagnostics(&self, source: &str) -> ScanDiagnostics {
        let mut raw_candidates = Vec::new();
        self.rules.scan(source, &mut raw_candidates);

        let raw_count = raw_candidates.len();
        let mut accepted = Vec::with_capacity(raw_count);

        for candidate in raw_candidates {
            let metadata = self.rules.metadata(candidate.rule_index());

            let Some(validation) = validate_candidate(
                metadata.validator(),
                source,
                candidate.start()..candidate.end(),
                metadata.confidence(),
            ) else {
                continue;
            };

            accepted.push(AcceptedCandidate {
                metadata,
                start: candidate.start(),
                end: candidate.end(),
                confidence: validation.confidence(),
            });
        }

        let accepted_count = accepted.len();
        normalize_candidates(&mut accepted);
        let normalized_count = accepted.len();

        ScanDiagnostics {
            raw_candidates: raw_count,
            accepted_candidates: accepted_count,
            normalized_candidates: normalized_count,
            findings: normalized_count,
        }
    }
}

impl Default for Scanner {
    fn default() -> Self {
        crate::builtins::current_scanner()
    }
}

/// Sorts accepted candidates and normalizes exact-span collisions.
///
/// Rules with the same matcher remain independently observable unless they have
/// the same rule identifier. This preserves the public contract that distinct
/// rules may report the same source span.
///
/// When a specialized validated rule and a lower-priority generic or custom
/// rule accept the exact same span, only candidates at the highest priority for
/// that span are retained.
///
/// Ranking inside an exact-span group is deterministic:
///
/// 1. greater rule priority;
/// 2. greater validation confidence;
/// 3. greater severity;
/// 4. lexicographically smaller rule identifier.
///
/// Partially overlapping spans are intentionally preserved.
fn normalize_candidates(candidates: &mut Vec<AcceptedCandidate<'_>>) {
    candidates.sort_unstable_by(|left, right| {
        left.start
            .cmp(&right.start)
            .then_with(|| left.end.cmp(&right.end))
            .then_with(|| right.metadata.priority().cmp(&left.metadata.priority()))
            .then_with(|| right.confidence.cmp(&left.confidence))
            .then_with(|| right.metadata.severity().cmp(&left.metadata.severity()))
            .then_with(|| {
                left.metadata
                    .id()
                    .as_str()
                    .cmp(right.metadata.id().as_str())
            })
    });

    candidates.dedup_by(|later, earlier| {
        later.same_span(*earlier) && later.metadata.id() == earlier.metadata.id()
    });

    let mut current_span = None;
    let mut highest_priority = 0;

    candidates.retain(|candidate| {
        let span = (candidate.start, candidate.end);

        if current_span != Some(span) {
            current_span = Some(span);
            highest_priority = candidate.metadata.priority();
            return true;
        }

        candidate.metadata.priority() == highest_priority
    });
}

/// Advances a one-based Unicode source position to `target`.
///
/// Candidates are already ordered by start byte, so the scanner traverses each
/// source byte range at most once while materializing accepted findings.
fn advance_position(
    source: &str,
    cursor: &mut usize,
    target: usize,
    line: &mut usize,
    column: &mut usize,
) {
    debug_assert!(target >= *cursor);
    debug_assert!(source.is_char_boundary(*cursor));
    debug_assert!(source.is_char_boundary(target));

    for character in source[*cursor..target].chars() {
        if character == '\n' {
            *line += 1;
            *column = 1;
        } else {
            *column += 1;
        }
    }

    *cursor = target;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Confidence, Rule, Severity, validators::dispatch::ValidatorKind};

    const DENSE_DIAGNOSTIC_SIZE: usize = 64 * 1_024;

    fn repeat_to_size(block: &str, size: usize) -> String {
        let mut source = String::with_capacity(size + block.len());

        while source.len() < size {
            source.push_str(block);
        }

        source.truncate(size);
        source
    }

    fn dense_diagnostic_source() -> String {
        const BLOCK: &str = concat!(
            "GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789\n",
            "STRIPE_SECRET_KEY=sk_live_AbCdEf0123456789_AbCdEf0123456789\n",
            "AWS_SECRET_ACCESS_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY\n",
            "POSTGRES_PASSWORD=CorrectHorseBatteryStaple!\n",
        );

        repeat_to_size(BLOCK, DENSE_DIAGNOSTIC_SIZE)
    }

    #[test]
    fn empty_builder_has_no_rules_or_findings() {
        let scanner = Scanner::builder().build().unwrap();

        assert!(scanner.is_empty());
        assert_eq!(scanner.rules_count(), 0);
        assert!(scanner.scan_source("anything").findings().is_empty());
    }

    #[test]
    fn default_scanner_contains_builtin_rules() {
        let scanner = Scanner::default();

        assert!(!scanner.is_empty());
        assert!(scanner.rules_count() > 0);
    }

    #[test]
    fn location_columns_count_unicode_scalars() {
        let scanner = Scanner::builder()
            .rule(Rule::literal("token", "secret", Severity::High))
            .build()
            .expect("scanner should compile");

        let report = scanner.scan_source("😀 secret");
        let location = report.findings()[0].location();

        assert_eq!(location.start(), 5);
        assert_eq!(location.end(), 11);
        assert_eq!(location.line(), 1);
        assert_eq!(location.column(), 3);
    }
    #[test]
    fn specialized_validator_rejects_invalid_candidate() {
        let scanner = Scanner::builder()
            .rule(
                Rule::prefix("github", "ghp_", Severity::Critical)
                    .with_validator(ValidatorKind::GitHub),
            )
            .build()
            .expect("scanner should compile");

        let report = scanner.scan_source("GITHUB_TOKEN=ghp_your_token_here");

        assert!(report.is_empty());
    }

    #[test]
    fn specialized_validator_accepts_and_overrides_confidence() {
        let token = "ghp_AbCdEf0123456789_AbCdEf0123456789";
        let scanner = Scanner::builder()
            .rule(
                Rule::prefix("github", "ghp_", Severity::Critical)
                    .with_validator(ValidatorKind::GitHub),
            )
            .build()
            .expect("scanner should compile");

        let report = scanner.scan_source(&format!("GITHUB_TOKEN={token}"));

        assert_eq!(report.len(), 1);
        assert_eq!(report.findings()[0].confidence(), Confidence::High);
    }

    #[test]
    fn unvalidated_custom_rule_preserves_existing_behavior() {
        let scanner = Scanner::builder()
            .rule(Rule::literal("custom", "custom-value", Severity::Medium))
            .build()
            .expect("scanner should compile");

        assert_eq!(scanner.scan_source("custom-value").len(), 1);
    }
    #[test]
    fn exact_duplicate_spans_are_collapsed() {
        let scanner = Scanner::builder()
            .rule(Rule::literal("duplicate", "secret", Severity::High))
            .rule(Rule::literal("duplicate", "secret", Severity::High))
            .build()
            .expect("scanner should compile");

        let report = scanner.scan_source("secret");

        assert_eq!(report.len(), 1);
        assert_eq!(report.findings()[0].rule_id().as_str(), "duplicate");
    }

    #[test]
    fn distinct_rules_with_identical_spans_are_preserved() {
        let scanner = Scanner::builder()
            .rule(Rule::literal("first", "secret", Severity::High))
            .rule(Rule::literal("second", "secret", Severity::High))
            .build()
            .expect("scanner should compile");

        let report = scanner.scan_source("secret");

        assert_eq!(report.len(), 2);
        assert_eq!(report.findings()[0].rule_id().as_str(), "first");
        assert_eq!(report.findings()[1].rule_id().as_str(), "second");
    }

    #[test]
    fn provider_specific_rule_wins_exact_span_collision() {
        let token = "ghp_AbCdEf0123456789_AbCdEf0123456789";
        let scanner = Scanner::builder()
            .rule(Rule::prefix("generic", "ghp_", Severity::Critical))
            .rule(
                Rule::prefix("github", "ghp_", Severity::Critical)
                    .with_validator(ValidatorKind::GitHub),
            )
            .build()
            .expect("scanner should compile");

        let report = scanner.scan_source(token);

        assert_eq!(report.len(), 1);
        assert_eq!(report.findings()[0].rule_id().as_str(), "github");
    }

    #[test]
    fn partially_overlapping_spans_are_preserved() {
        let scanner = Scanner::builder()
            .rule(Rule::literal("whole", "secret-value", Severity::High))
            .rule(Rule::literal("part", "secret", Severity::Medium))
            .build()
            .expect("scanner should compile");

        let report = scanner.scan_source("secret-value");

        assert_eq!(report.len(), 2);
    }
    #[test]
    fn dense_fixture_diagnostics() {
        let scanner = Scanner::default();
        let source = dense_diagnostic_source();
        let diagnostics = scanner.diagnostics(&source);
        let report = scanner.scan_source(&source);

        println!(
            "dense diagnostics: bytes={}, raw={}, accepted={}, rejected={}, normalized={}, collapsed={}, findings={}",
            source.len(),
            diagnostics.raw_candidates,
            diagnostics.accepted_candidates,
            diagnostics.rejected_candidates(),
            diagnostics.normalized_candidates,
            diagnostics.collapsed_candidates(),
            diagnostics.findings,
        );

        assert_eq!(diagnostics.findings, report.len());
        assert!(diagnostics.raw_candidates >= diagnostics.accepted_candidates);
        assert!(diagnostics.accepted_candidates >= diagnostics.normalized_candidates);
        assert!(diagnostics.findings > 0);
    }
}
