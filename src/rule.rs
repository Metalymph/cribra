use crate::severity::Severity;
use std::{fmt, sync::Arc};

use regex::Regex;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct RuleId(Arc<str>);

impl RuleId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for RuleId {
    fn from(value: String) -> Self {
        Self(Arc::from(value))
    }
}

impl From<&str> for RuleId {
    fn from(value: &str) -> Self {
        Self(Arc::from(value))
    }
}

impl fmt::Display for RuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum RuleKind {
    Literal,
    Prefix,
    Suffix,
    Pattern,
}

/// Static definition of a built-in rule.
///
/// `RuleSpec` contains only `'static` data, so built-in rules can be exposed
/// as compile-time constants without allocation.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct RuleSpec {
    pub id: &'static str,
    pub kind: RuleKind,
    pub value: &'static str,
    pub severity: Severity,
}

impl RuleSpec {
    #[must_use]
    pub const fn literal(
        id: &'static str,
        value: &'static str,
        severity: Severity,
    ) -> Self {
        Self {
            id,
            kind: RuleKind::Literal,
            value,
            severity,
        }
    }

    #[must_use]
    pub const fn prefix(
        id: &'static str,
        value: &'static str,
        severity: Severity,
    ) -> Self {
        Self {
            id,
            kind: RuleKind::Prefix,
            value,
            severity,
        }
    }

    #[must_use]
    pub const fn suffix(
        id: &'static str,
        value: &'static str,
        severity: Severity,
    ) -> Self {
        Self {
            id,
            kind: RuleKind::Suffix,
            value,
            severity,
        }
    }

    #[must_use]
    pub const fn pattern(
        id: &'static str,
        value: &'static str,
        severity: Severity,
    ) -> Self {
        Self {
            id,
            kind: RuleKind::Pattern,
            value,
            severity,
        }
    }

    #[must_use]
    pub fn to_rule(self) -> Result<Rule, RuleError> {
        match self.kind {
            RuleKind::Literal => Ok(Rule::literal(self.id, self.value, self.severity)),
            RuleKind::Prefix => Ok(Rule::prefix(self.id, self.value, self.severity)),
            RuleKind::Suffix => Ok(Rule::suffix(self.id, self.value, self.severity)),
            RuleKind::Pattern => Rule::pattern(self.id, self.value, self.severity),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Matcher {
    Literal(Box<str>),
    Prefix(Box<str>),
    Suffix(Box<str>),
    Pattern(Regex),
}

/// Owned rule configuration accepted by a scanner.
#[derive(Debug, Clone)]
pub struct Rule {
    pub(crate) id: RuleId,
    pub(crate) severity: Severity,
    pub(crate) matcher: Matcher,
}

impl Rule {
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

    /// Creates a rule that matches a prefix.
    #[must_use]
    pub fn prefix(
        id: impl Into<RuleId>,
        prefix: impl Into<Box<str>>,
        severity: Severity,
    ) -> Self {
        Self {
            id: id.into(),
            severity,
            matcher: Matcher::Prefix(prefix.into()),
        }
    }

    /// Creates a rule that matches a suffix.
    #[must_use]
    pub fn suffix(
        id: impl Into<RuleId>,
        suffix: impl Into<Box<str>>,
        severity: Severity,
    ) -> Self {
        Self {
            id: id.into(),
            severity,
            matcher: Matcher::Suffix(suffix.into()),
        }
    }

    /// Creates a rule that matches a pattern.
    ///
    /// # Errors
    ///
    /// Returns an error if the pattern is invalid.
    pub fn pattern(
        id: impl Into<RuleId>,
        pattern: &str,
        severity: Severity,
    ) -> Result<Self, RuleError> {
        let pattern = Regex::new(pattern).map_err(RuleError::InvalidPattern)?;
    
        Ok(Self {
            id: id.into(),
            severity,
            matcher: Matcher::Pattern(pattern),
        })
    }

    #[must_use]
    pub fn id(&self) -> &RuleId {
        &self.id
    }

    #[must_use]
    pub const fn severity(&self) -> Severity {
        self.severity
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.id.fmt(f)
    }
}

#[derive(Debug)]
pub enum RuleError {
    InvalidPattern(regex::Error),
}

impl std::fmt::Display for RuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPattern(error) => write!(f, "invalid rule pattern: {error}"),
        }
    }
}

impl std::error::Error for RuleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidPattern(error) => Some(error),
        }
    }
}