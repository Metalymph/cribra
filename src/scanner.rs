use crate::{
    compiled_rule::{CompiledRuleSet, InternalFinding},
    finding::Finding,
    location::Location,
    report::ScanReport,
    scanner_builder::ScannerBuilder,
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
    /// 1. execute every compiled matcher group into one internal finding
    ///    buffer;
    /// 2. sort matches deterministically by byte span and rule identifier;
    /// 3. resolve one-based line and Unicode-scalar column coordinates in one
    ///    forward pass;
    /// 4. materialize the public [`Finding`] values.
    ///
    /// `start` and `end` locations remain zero-based byte offsets. `line` and
    /// `column` are one-based, and columns count Unicode scalar values rather
    /// than UTF-8 bytes.
    #[must_use]
    pub fn scan(&self, source: &str) -> ScanReport {
        let mut internal = Vec::new();
        self.rules.scan(source, &mut internal);

        internal.sort_unstable_by(|left, right| {
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

        let positions = resolve_positions(source, &internal);

        let findings = internal
            .into_iter()
            .zip(positions)
            .map(|(finding, (line, column))| {
                let metadata = self.rules.metadata(finding.rule_index());
                let mut location = Location::from_span(finding.start(), finding.end());
                location.set_position(line, column);

                Finding::new(
                    metadata.id().clone(),
                    location,
                    metadata.severity(),
                    metadata.confidence(),
                )
            })
            .collect();

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

/// Resolves positions for findings already ordered by their start byte.
///
/// The function traverses the source only once. It returns positions separately
/// from [`InternalFinding`] so the hot-path representation remains limited to a
/// compact rule index and byte span.
fn resolve_positions(source: &str, findings: &[InternalFinding]) -> Vec<(usize, usize)> {
    let mut positions = Vec::with_capacity(findings.len());

    let mut cursor = 0;
    let mut line = 1;
    let mut column = 1;

    for finding in findings {
        debug_assert!(finding.start() >= cursor);
        debug_assert!(source.is_char_boundary(finding.start()));
        debug_assert!(source.is_char_boundary(finding.end()));

        for character in source[cursor..finding.start()].chars() {
            if character == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }

        positions.push((line, column));
        cursor = finding.start();
    }

    positions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Rule, Severity};

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
}
