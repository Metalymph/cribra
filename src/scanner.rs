use crate::{
    compiled_rule::CompiledRuleSet, finding::Finding, location::Location, report::ScanReport,
    scanner_builder::ScannerBuilder, validators::dispatch::validate_candidate,
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
#[derive(Debug)]
pub struct Scanner {
    rules: CompiledRuleSet,
}

impl Scanner {
    pub(crate) const fn new(rules: CompiledRuleSet) -> Self {
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
    /// 2. sort candidates deterministically by byte span and rule identifier;
    /// 3. dispatch only the validator selected by each rule;
    /// 4. reject invalid candidates before public materialization;
    /// 5. resolve one-based line and Unicode-scalar column coordinates in one
    ///    forward pass;
    /// 6. materialize the accepted public [`Finding`] values.
    ///
    /// `start` and `end` locations remain zero-based byte offsets. `line` and
    /// `column` are one-based, and columns count Unicode scalar values rather
    /// than UTF-8 bytes.
    #[must_use]
    pub fn scan(&self, source: &str) -> ScanReport {
        let mut candidates = Vec::new();
        self.rules.scan(source, &mut candidates);

        candidates.sort_unstable_by(|left, right| {
            left.start()
                .cmp(&right.start())
                .then_with(|| left.end().cmp(&right.end()))
                .then_with(|| {
                    self.rules
                        .metadata(left.rule_index())
                        .id()
                        .as_str()
                        .cmp(self.rules.metadata(right.rule_index()).id().as_str())
                })
        });

        let mut findings = Vec::with_capacity(candidates.len());
        let mut cursor = 0;
        let mut line = 1;
        let mut column = 1;

        for candidate in candidates {
            let metadata = self.rules.metadata(candidate.rule_index());

            let Some(validation) = validate_candidate(
                metadata.validator(),
                source,
                candidate.start()..candidate.end(),
                metadata.confidence(),
            ) else {
                continue;
            };

            advance_position(
                source,
                &mut cursor,
                candidate.start(),
                &mut line,
                &mut column,
            );

            let mut location = Location::from_span(candidate.start(), candidate.end());
            location.set_position(line, column);

            findings.push(Finding::new(
                metadata.id().clone(),
                location,
                metadata.severity(),
                validation.confidence(),
            ));
        }

        ScanReport::new(findings)
    }
}

impl Default for Scanner {
    fn default() -> Self {
        Self::builder()
            .build()
            .expect("an empty scanner configuration must compile")
    }
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

    #[test]
    fn empty_scanner_has_no_rules_or_findings() {
        let scanner = Scanner::default();

        assert!(scanner.is_empty());
        assert_eq!(scanner.rules_count(), 0);
        assert!(scanner.scan("anything").findings().is_empty());
    }

    #[test]
    fn location_columns_count_unicode_scalars() {
        let scanner = Scanner::builder()
            .rule(Rule::literal("token", "secret", Severity::High))
            .build()
            .expect("scanner should compile");

        let report = scanner.scan("😀 secret");
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

        let report = scanner.scan("GITHUB_TOKEN=ghp_your_token_here");

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

        let report = scanner.scan(&format!("GITHUB_TOKEN={token}"));

        assert_eq!(report.len(), 1);
        assert_eq!(report.findings()[0].confidence(), Confidence::High);
    }

    #[test]
    fn unvalidated_custom_rule_preserves_existing_behavior() {
        let scanner = Scanner::builder()
            .rule(Rule::literal("custom", "custom-value", Severity::Medium))
            .build()
            .expect("scanner should compile");

        assert_eq!(scanner.scan("custom-value").len(), 1);
    }
}
