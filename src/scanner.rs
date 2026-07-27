use crate::{
    confidence::Confidence,
    finding::Finding,
    location::Location,
    rule::{Matcher, Rule, RuleSpec},
    scanner_builder::ScannerBuilder,
};

/// A report produced by a scan.
#[derive(Debug, Default)]
pub struct ScanReport {
    findings: Vec<Finding>,
}

impl ScanReport {
    #[must_use]
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.findings.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.findings.len()
    }
}

/// A scanner configured with an immutable set of rules.
#[derive(Debug, Default)]
pub struct Scanner {
    rules: Vec<Rule>,
}

impl Scanner {
    pub(crate) fn new(rules: Vec<Rule>) -> Self {
        Self { rules }
    }

    /// Creates an empty scanner builder.
    #[must_use]
    pub const fn builder() -> ScannerBuilder {
        ScannerBuilder::new()
    }

    /// Scans UTF-8 text using the configured rules.
    #[must_use]
    pub fn scan(&self, source: &str) -> ScanReport {
        // Vec::new() performs no heap allocation.
        let mut findings = Vec::new();

        for rule in &self.rules {
            scan_rule(rule, source, &mut findings);
        }

        if findings.is_empty() {
            return ScanReport { findings };
        }

        // Parallel execution must never affect the public ordering.
        findings.sort_unstable_by(|left, right| {
            left.location()
                .start()
                .cmp(&right.location().start())
                .then_with(|| left.location().end().cmp(&right.location().end()))
                .then_with(|| left.rule_id().as_str().cmp(right.rule_id().as_str()))
        });

        populate_line_columns(source, &mut findings);

        ScanReport { findings }
    }

    #[must_use]
    pub fn rules_count(&self) -> usize {
        self.rules.len()
    }
}

/// Built-in Silens Scan rules.
pub mod builtins {
    use super::RuleSpec;
    use crate::severity::Severity;

    pub const STRIPE_SECRET_KEY: RuleSpec =
        RuleSpec::prefix("stripe.secret-key", "sk_live_", Severity::Critical);

    pub const GITHUB_PAT: RuleSpec = RuleSpec::prefix(
        "github.personal-access-token",
        "github_pat_",
        Severity::Critical,
    );

    pub const ALL: &[RuleSpec] = &[STRIPE_SECRET_KEY, GITHUB_PAT];
}

fn scan_rule(rule: &Rule, source: &str, findings: &mut Vec<Finding>) {
    match &rule.matcher {
        Matcher::Literal(value) => {
            scan_literal(rule, source, value, findings);
        }
        Matcher::Prefix(value) => {
            scan_prefix(rule, source, value, findings);
        }
        Matcher::Suffix(value) => {
            scan_suffix(rule, source, value, findings);
        }
        Matcher::Pattern(pattern) => {
            scan_pattern(rule, source, pattern, findings);
        }
    }
}

fn scan_literal(rule: &Rule, source: &str, literal: &str, findings: &mut Vec<Finding>) {
    if literal.is_empty() {
        return;
    }

    for (start, _) in source.match_indices(literal) {
        push_finding(findings, rule, start, start + literal.len());
    }
}

fn scan_prefix(rule: &Rule, source: &str, prefix: &str, findings: &mut Vec<Finding>) {
    if prefix.is_empty() {
        return;
    }

    for (start, _) in source.match_indices(prefix) {
        if start > 0 && is_token_byte(source.as_bytes()[start - 1]) {
            continue;
        }

        let mut end = start + prefix.len();
        let bytes = source.as_bytes();

        while end < bytes.len() && is_token_byte(bytes[end]) {
            end += 1;
        }

        // println!(
        //     "match at {}, prev={:?}",
        //     start,
        //     start.checked_sub(1).map(|i| source.as_bytes()[i] as char),
        // );
        push_finding(findings, rule, start, end);
    }
}

fn scan_suffix(rule: &Rule, source: &str, suffix: &str, findings: &mut Vec<Finding>) {
    if suffix.is_empty() {
        return;
    }

    for (suffix_start, _) in source.match_indices(suffix) {
        let end = suffix_start + suffix.len();

        if end < source.len() && is_token_byte(source.as_bytes()[end]) {
            continue;
        }

        let bytes = source.as_bytes();
        let mut start = suffix_start;

        while start > 0 && is_token_byte(bytes[start - 1]) {
            start -= 1;
        }

        // println!(
        //     "suffix start {}, next={:?}",
        //     end,
        //     source.as_bytes().get(end).map(|b| *b as char),
        // );
        push_finding(findings, rule, start, end);
    }
}

fn scan_pattern(rule: &Rule, source: &str, pattern: &regex::Regex, findings: &mut Vec<Finding>) {
    for matched in pattern.find_iter(source) {
        push_finding(findings, rule, matched.start(), matched.end());
    }
}

fn push_finding(findings: &mut Vec<Finding>, rule: &Rule, start: usize, end: usize) {
    findings.push(Finding::new(
        rule.id.clone(),
        Location::from_span(start, end),
        rule.severity,
        Confidence::High,
    ));
}

/// Characters accepted as part of the token surrounding a prefix/suffix.
///
/// The scanner deliberately keeps this ASCII-only. Secret formats with
/// different lexical rules should use a Pattern rule.
#[inline]
const fn is_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

/// Populates line/column position for each finding in-place.
fn populate_line_columns(source: &str, findings: &mut [Finding]) {
    let mut cursor = 0;
    let mut line = 1;
    let mut column = 1;

    for finding in findings {
        let start = finding.location().start();

        if start > cursor {
            for ch in source[cursor..start].chars() {
                if ch == '\n' {
                    line += 1;
                    column = 1;
                } else {
                    column += 1;
                }
            }

            cursor = start;
        }

        finding.location_mut().set_position(line, column);
    }
}
