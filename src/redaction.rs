//! Safe redaction marker used by Cribra consumers.
//!
//! A [`Redaction`] stores only replacement text. It never stores the original
//! matched source or secret.

use std::fmt;

/// Replacement text used when presenting or exporting a detected span.
///
/// `Redaction` deliberately contains no source text and no reference to a
/// [`Finding`](crate::Finding). Consumers apply the replacement to the source
/// only at the boundary where redacted output is produced.
///
/// The default replacement is `[REDACTED]`.
///
/// # Examples
///
/// ```
/// use cribra::Redaction;
///
/// let redaction = Redaction::hidden();
/// assert_eq!(redaction.as_str(), "[REDACTED]");
/// ```
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Redaction(Box<str>);

impl Redaction {
    /// Creates a redaction from caller-provided replacement text.
    ///
    /// The supplied value is replacement text only. The matched secret should
    /// never be passed to this constructor.
    #[must_use]
    pub fn new(replacement: impl Into<Box<str>>) -> Self {
        Self(replacement.into())
    }

    /// Creates the standard fully hidden redaction.
    #[must_use]
    pub fn hidden() -> Self {
        Self::default()
    }

    /// Returns the replacement text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Redaction {
    fn default() -> Self {
        Self::new("[REDACTED]")
    }
}

impl AsRef<str> for Redaction {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for Redaction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl From<&str> for Redaction {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for Redaction {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<Box<str>> for Redaction {
    fn from(value: Box<str>) -> Self {
        Self(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_uses_safe_default_marker() {
        assert_eq!(Redaction::hidden().as_str(), "[REDACTED]");
    }

    #[test]
    fn supports_custom_replacement_text() {
        let redaction = Redaction::new("***");

        assert_eq!(redaction.as_str(), "***");
        assert_eq!(redaction.to_string(), "***");
    }
}
