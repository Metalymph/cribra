use std::{borrow::Borrow, collections::HashSet, error::Error, fmt, sync::Arc};

use crate::{
    compiled_rule::CompiledRuleSet,
    rule::{Rule, RuleError, RuleId, RuleSpec},
    scanner::Scanner,
};

/// Error returned when a [`Scanner`] cannot be compiled from its configured
/// rules.
///
/// Scanner construction is a separate phase from scanning. All validation and
/// matcher compilation happen in [`ScannerBuilder::build`], allowing
/// [`Scanner::scan`](crate::Scanner::scan) to remain deterministic and free of
/// rule-construction errors.
#[derive(Debug)]
pub enum ScannerBuildError {
    /// A built-in [`RuleSpec`] could not be converted into an owned [`Rule`].
    Rule(RuleError),

    /// A rule uses an empty identifier.
    ///
    /// Rule identifiers are part of the stable machine-readable finding
    /// contract and therefore cannot be empty.
    EmptyRuleId,

    /// Two configured rules use the same stable identifier.
    ///
    /// Rule identifiers are scanner-wide identities. Duplicate identifiers are
    /// rejected across custom rules and built-in rules so metadata lookup and
    /// explainability remain unambiguous.
    DuplicateRuleId {
        /// Identifier used by more than one configured rule.
        rule_id: RuleId,
    },

    /// A literal, prefix, or suffix matcher is empty.
    ///
    /// Empty matchers are rejected because they either match every source
    /// position or otherwise have no useful detection semantics.
    EmptyMatcher {
        /// Identifier of the invalid rule.
        rule_id: RuleId,
    },

    /// The configured rule count exceeds the capacity of the internal compact
    /// rule index.
    ///
    /// The exact internal integer representation is deliberately not part of
    /// the public API and may change after layout and benchmark measurements.
    TooManyRules,

    /// The shared multi-pattern automaton could not be built.
    AutomatonBuild(aho_corasick::BuildError),
}

impl fmt::Display for ScannerBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rule(error) => write!(formatter, "could not construct rule: {error}"),
            Self::EmptyRuleId => formatter.write_str("rule identifier cannot be empty"),
            Self::DuplicateRuleId { rule_id } => {
                write!(formatter, "duplicate rule identifier `{rule_id}`")
            }
            Self::EmptyMatcher { rule_id } => {
                write!(formatter, "rule `{rule_id}` uses an empty matcher")
            }
            Self::TooManyRules => {
                formatter.write_str("configured rule count exceeds the scanner limit")
            }
            Self::AutomatonBuild(error) => {
                write!(
                    formatter,
                    "could not compile multi-pattern matcher: {error}"
                )
            }
        }
    }
}

impl Error for ScannerBuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Rule(error) => Some(error),
            Self::AutomatonBuild(error) => Some(error),
            Self::EmptyRuleId
            | Self::DuplicateRuleId { .. }
            | Self::EmptyMatcher { .. }
            | Self::TooManyRules => None,
        }
    }
}

impl From<RuleError> for ScannerBuildError {
    fn from(error: RuleError) -> Self {
        Self::Rule(error)
    }
}

/// Builder used to configure and compile an immutable [`Scanner`].
///
/// A new builder starts with no rules. Built-in rule specifications and custom
/// owned rules can be combined in any order. Calling [`build`](Self::build)
/// converts built-ins into owned rules, validates the full configuration, and
/// compiles the private execution plan.
///
/// Rule identifiers are unique within one scanner. This makes a rule ID a
/// reliable application-facing identity for querying, metadata lookup and
/// explainability.
///
/// # Examples
///
/// ```
/// use silens_scan::{Rule, Scanner, Severity};
///
/// let scanner = Scanner::builder()
///     .rule(Rule::literal(
///         "example.exact-token",
///         "internal_exact_token",
///         Severity::High,
///     ))
///     .rule(Rule::prefix(
///         "example.internal-token",
///         "internal_live_",
///         Severity::Critical,
///     ))
///     .build()?;
///
/// assert_eq!(scanner.rules_count(), 2);
/// # Ok::<(), silens_scan::ScannerBuildError>(())
/// ```
#[derive(Debug, Default)]
pub struct ScannerBuilder {
    rules: Vec<Rule>,
    builtin_rules: Vec<RuleSpec>,
}

impl ScannerBuilder {
    /// Creates an empty scanner builder.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            rules: Vec::new(),
            builtin_rules: Vec::new(),
        }
    }

    /// Adds one built-in rule specification.
    ///
    /// The specification is converted into an owned [`Rule`] during
    /// [`build`](Self::build).
    #[must_use]
    pub fn builtin(mut self, rule: RuleSpec) -> Self {
        self.builtin_rules.push(rule);
        self
    }

    /// Adds multiple built-in rule specifications.
    ///
    /// Both owned iterators of [`RuleSpec`] and borrowed slices such as
    /// [`crate::builtins::CURRENT`] are accepted.
    #[must_use]
    pub fn builtins<I>(mut self, rules: I) -> Self
    where
        I: IntoIterator,
        I::Item: Borrow<RuleSpec>,
    {
        self.builtin_rules
            .extend(rules.into_iter().map(|rule| *rule.borrow()));
        self
    }

    /// Adds one custom owned rule.
    ///
    /// Custom rules use matcher-authoritative semantics and therefore expose
    /// [`crate::DetectionMode::MatcherOnly`] through their metadata.
    #[must_use]
    pub fn rule(mut self, rule: Rule) -> Self {
        self.rules.push(rule);
        self
    }

    /// Adds multiple custom owned rules.
    ///
    /// Input order is preserved in the compiled metadata table.
    #[must_use]
    pub fn rules<I>(mut self, rules: I) -> Self
    where
        I: IntoIterator<Item = Rule>,
    {
        self.rules.extend(rules);
        self
    }

    /// Validates and compiles the configured rules into an immutable scanner.
    ///
    /// Compilation performs all work that should not occur in the scan hot
    /// path, including:
    ///
    /// - conversion of static built-in specifications into owned rules;
    /// - validation of rule identifiers and matcher values;
    /// - rejection of duplicate rule identifiers across the complete scanner;
    /// - construction of the shared multi-pattern automaton;
    /// - grouping of suffix and regular-expression matchers;
    /// - construction of the immutable rule metadata table.
    ///
    /// The returned scanner can be reused for any number of input strings.
    ///
    /// # Errors
    ///
    /// Returns [`ScannerBuildError`] when a rule is invalid, a rule identifier
    /// is duplicated, or the private execution plan cannot be compiled.
    pub fn build(mut self) -> Result<Scanner, ScannerBuildError> {
        self.rules.reserve(self.builtin_rules.len());

        for specification in self.builtin_rules {
            self.rules.push(specification.to_rule()?);
        }

        validate_unique_rule_ids(&self.rules)?;

        let rules = CompiledRuleSet::compile(self.rules)?;
        Ok(Scanner::new(Arc::new(rules)))
    }
}

fn validate_unique_rule_ids(rules: &[Rule]) -> Result<(), ScannerBuildError> {
    let mut seen = HashSet::with_capacity(rules.len());

    for rule in rules {
        if !seen.insert(rule.id().clone()) {
            return Err(ScannerBuildError::DuplicateRuleId {
                rule_id: rule.id().clone(),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Severity, builtins};

    #[test]
    fn builtins_accept_borrowed_catalog_slice() {
        let scanner = ScannerBuilder::new()
            .builtins(builtins::CURRENT)
            .build()
            .expect("borrowed built-in catalog should compile");

        assert_eq!(scanner.rules_count(), builtins::CURRENT.len());
    }

    #[test]
    fn duplicate_custom_rule_ids_are_rejected() {
        let error = ScannerBuilder::new()
            .rules([
                Rule::literal("acme.shared", "FIRST", Severity::High),
                Rule::literal("acme.shared", "SECOND", Severity::Critical),
            ])
            .build()
            .expect_err("duplicate custom rule identifiers should fail");

        assert!(matches!(
            error,
            ScannerBuildError::DuplicateRuleId { ref rule_id }
                if rule_id.as_str() == "acme.shared"
        ));
    }

    #[test]
    fn custom_rule_cannot_shadow_builtin_identifier() {
        let builtin = builtins::CURRENT[0];

        let error = ScannerBuilder::new()
            .builtin(builtin)
            .rule(Rule::literal(
                builtin.id(),
                "CUSTOM_VALUE",
                Severity::Critical,
            ))
            .build()
            .expect_err("custom rule should not shadow built-in identity");

        assert!(matches!(
            error,
            ScannerBuildError::DuplicateRuleId { ref rule_id }
                if rule_id.as_str() == builtin.id()
        ));
    }
}
