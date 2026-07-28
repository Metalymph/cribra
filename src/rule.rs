//! Rule definitions accepted by [`ScannerBuilder`](crate::ScannerBuilder).
//!
//! Public rules are declarative configuration values. They describe what the
//! scanner should detect, but they do not execute matching themselves.
//!
//! During [`ScannerBuilder::build`](crate::ScannerBuilder::build), every rule is
//! validated and moved into the private compiled execution engine. This keeps
//! rule construction ergonomic while allowing the scanner internals to evolve
//! without exposing matcher implementation details.

use std::{error::Error, fmt, sync::Arc};

use regex::Regex;

use crate::severity::Severity;

/// Stable identifier assigned to a detection rule.
///
/// A rule identifier is copied cheaply because its string storage is shared.
/// It should be concise, deterministic and suitable for structured output.
///
/// Identifiers are validated when a scanner is built. Empty identifiers are
/// rejected by [`ScannerBuilder::build`](crate::ScannerBuilder::build).
///
/// # Examples
///
/// ```
/// use silens_scan::RuleId;
///
/// let id = RuleId::from("github-personal-access-token");
/// assert_eq!(id.as_str(), "github-personal-access-token");
/// ```
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct RuleId(Arc<str>);

impl RuleId {
    /// Returns this identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for RuleId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl From<&str> for RuleId {
    fn from(value: &str) -> Self {
        Self(Arc::from(value))
    }
}

impl From<String> for RuleId {
    fn from(value: String) -> Self {
        Self(Arc::from(value))
    }
}

impl From<Box<str>> for RuleId {
    fn from(value: Box<str>) -> Self {
        Self(Arc::from(value))
    }
}

impl From<Arc<str>> for RuleId {
    fn from(value: Arc<str>) -> Self {
        Self(value)
    }
}

impl fmt::Display for RuleId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Declarative matching strategy used by a static [`RuleSpec`].
///
/// The concrete matcher representation used during scanning is private.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum RuleKind {
    /// Match the exact text wherever it occurs.
    Literal,

    /// Match a token beginning with the configured text.
    ///
    /// The compiled engine requires a token boundary before the prefix and
    /// extends the finding through the remaining token characters.
    Prefix,

    /// Match a token ending with the configured text.
    ///
    /// The compiled engine requires a token boundary after the suffix and
    /// extends the finding backwards through preceding token characters.
    Suffix,

    /// Match spans produced by a regular expression.
    Pattern,
}

/// Allocation-free definition of a built-in rule.
///
/// `RuleSpec` stores only `'static` data and can therefore be declared as a
/// `const`. It is intended for built-in rule catalogs. User-defined runtime
/// configuration should normally use [`Rule`] directly.
///
/// A specification is converted into an owned [`Rule`] when a scanner is
/// built. Regular-expression specifications can fail during this conversion
/// when their pattern is invalid.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct RuleSpec {
    id: &'static str,
    kind: RuleKind,
    value: &'static str,
    severity: Severity,
}

impl RuleSpec {
    /// Defines a static exact-literal rule.
    #[must_use]
    pub const fn literal(id: &'static str, literal: &'static str, severity: Severity) -> Self {
        Self {
            id,
            kind: RuleKind::Literal,
            value: literal,
            severity,
        }
    }

    /// Defines a static token-prefix rule.
    #[must_use]
    pub const fn prefix(id: &'static str, prefix: &'static str, severity: Severity) -> Self {
        Self {
            id,
            kind: RuleKind::Prefix,
            value: prefix,
            severity,
        }
    }

    /// Defines a static token-suffix rule.
    #[must_use]
    pub const fn suffix(id: &'static str, suffix: &'static str, severity: Severity) -> Self {
        Self {
            id,
            kind: RuleKind::Suffix,
            value: suffix,
            severity,
        }
    }

    /// Defines a static regular-expression rule.
    ///
    /// The expression is compiled only when this specification is converted
    /// into an owned [`Rule`].
    #[must_use]
    pub const fn pattern(id: &'static str, pattern: &'static str, severity: Severity) -> Self {
        Self {
            id,
            kind: RuleKind::Pattern,
            value: pattern,
            severity,
        }
    }

    /// Returns the stable identifier assigned to this specification.
    #[must_use]
    pub const fn id(self) -> &'static str {
        self.id
    }

    /// Returns the declarative matching strategy.
    #[must_use]
    pub const fn kind(self) -> RuleKind {
        self.kind
    }

    /// Returns the literal, prefix, suffix or regular-expression source.
    #[must_use]
    pub const fn value(self) -> &'static str {
        self.value
    }

    /// Returns the severity assigned to findings from this rule.
    #[must_use]
    pub const fn severity(self) -> Severity {
        self.severity
    }

    /// Converts this static specification into an owned rule.
    ///
    /// # Errors
    ///
    /// Returns [`RuleError::InvalidPattern`] when this is a pattern
    /// specification whose regular expression cannot be compiled.
    pub fn to_rule(self) -> Result<Rule, RuleError> {
        match self.kind {
            RuleKind::Literal => Ok(Rule::literal(self.id, self.value, self.severity)),
            RuleKind::Prefix => Ok(Rule::prefix(self.id, self.value, self.severity)),
            RuleKind::Suffix => Ok(Rule::suffix(self.id, self.value, self.severity)),
            RuleKind::Pattern => Rule::pattern(self.id, self.value, self.severity),
        }
    }
}

/// Private owned matcher retained by a [`Rule`] before scanner compilation.
#[derive(Debug, Clone)]
pub(crate) enum Matcher {
    Literal(Box<str>),
    Prefix(Box<str>),
    Suffix(Box<str>),
    Pattern(Regex),
}

/// Owned declarative detection rule.
///
/// A `Rule` contains configuration only. Calling a constructor does not scan
/// text, and no matcher is rebuilt during [`Scanner::scan`](crate::Scanner::scan).
///
/// Literal, prefix and suffix rules are infallible to construct. Empty values
/// are rejected later when the complete scanner configuration is validated.
/// Pattern rules compile their regular expression immediately and therefore
/// return a [`Result`].
#[derive(Debug, Clone)]
pub struct Rule {
    pub(crate) id: RuleId,
    pub(crate) severity: Severity,
    pub(crate) matcher: Matcher,
}

impl Rule {
    /// Creates a rule that reports every exact occurrence of `literal`.
    ///
    /// Literal matching does not impose token-boundary semantics.
    #[must_use]
    pub fn literal(
        id: impl Into<RuleId>,
        literal: impl Into<Box<str>>,
        severity: Severity,
    ) -> Self {
        Self {
            id: id.into(),
            severity,
            matcher: Matcher::Literal(literal.into()),
        }
    }

    /// Creates a rule that reports complete tokens beginning with `prefix`.
    ///
    /// The prefix must begin at a token boundary. After a candidate is found,
    /// the compiled engine extends the match through ASCII alphanumeric
    /// characters, `_` and `-`.
    #[must_use]
    pub fn prefix(id: impl Into<RuleId>, prefix: impl Into<Box<str>>, severity: Severity) -> Self {
        Self {
            id: id.into(),
            severity,
            matcher: Matcher::Prefix(prefix.into()),
        }
    }

    /// Creates a rule that reports complete tokens ending with `suffix`.
    ///
    /// The suffix must end at a token boundary. The compiled engine extends
    /// the match backwards through ASCII alphanumeric characters, `_` and `-`.
    #[must_use]
    pub fn suffix(id: impl Into<RuleId>, suffix: impl Into<Box<str>>, severity: Severity) -> Self {
        Self {
            id: id.into(),
            severity,
            matcher: Matcher::Suffix(suffix.into()),
        }
    }

    /// Creates a regular-expression rule.
    ///
    /// The expression is compiled once during rule construction and moved into
    /// the scanner's compiled rule set. It is never recompiled by `scan`.
    ///
    /// Each non-overlapping span returned by the regex engine becomes one
    /// finding.
    ///
    /// # Errors
    ///
    /// Returns [`RuleError::InvalidPattern`] when `pattern` is not a valid
    /// regular expression.
    pub fn pattern(
        id: impl Into<RuleId>,
        pattern: impl AsRef<str>,
        severity: Severity,
    ) -> Result<Self, RuleError> {
        let pattern = Regex::new(pattern.as_ref()).map_err(RuleError::InvalidPattern)?;

        Ok(Self {
            id: id.into(),
            severity,
            matcher: Matcher::Pattern(pattern),
        })
    }

    /// Returns this rule's stable identifier.
    #[must_use]
    pub fn id(&self) -> &RuleId {
        &self.id
    }

    /// Returns the severity assigned to findings produced by this rule.
    #[must_use]
    pub const fn severity(&self) -> Severity {
        self.severity
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.id.fmt(formatter)
    }
}

/// Error produced while constructing an individual [`Rule`].
///
/// Errors that involve validating or compiling a complete collection of rules
/// are represented by [`ScannerBuildError`](crate::ScannerBuildError).
#[derive(Debug)]
pub enum RuleError {
    /// The supplied regular expression is invalid.
    InvalidPattern(regex::Error),
}

impl fmt::Display for RuleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPattern(error) => write!(formatter, "invalid rule pattern: {error}"),
        }
    }
}

impl Error for RuleError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidPattern(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_id_supports_owned_and_borrowed_strings() {
        let borrowed = RuleId::from("borrowed");
        let owned = RuleId::from(String::from("owned"));

        assert_eq!(borrowed.as_str(), "borrowed");
        assert_eq!(owned.as_str(), "owned");
    }

    #[test]
    fn static_literal_spec_converts_to_owned_rule() {
        let rule = RuleSpec::literal("literal", "SECRET", Severity::High)
            .to_rule()
            .expect("literal specification should convert");

        assert_eq!(rule.id().as_str(), "literal");
        assert_eq!(rule.severity(), Severity::High);
    }

    #[test]
    fn invalid_pattern_is_rejected_during_rule_construction() {
        let error =
            Rule::pattern("invalid", "(", Severity::High).expect_err("invalid regex should fail");

        assert!(matches!(error, RuleError::InvalidPattern(_)));
    }
}
