//! Confidence assigned to findings produced by detection rules.

use core::fmt;

/// Indicates how reliable a detection is.
///
/// Confidence represents how certain the scanner is that a reported finding is
/// a true positive.
///
/// It is independent from [`Severity`](crate::Severity). A finding may be
/// highly severe but have low confidence, or vice versa.
///
/// The ordering of the variants follows increasing confidence:
///
/// ```text
/// Low < Medium < High
/// ```
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Confidence {
    /// Detection is plausible but may require manual verification.
    Low,

    /// Detection is reasonably reliable.
    Medium,

    /// Detection has a very low probability of being a false positive.
    High,
}

impl Confidence {
    /// Returns `true` when this confidence level is considered trustworthy.
    #[must_use]
    pub const fn is_high(self) -> bool {
        matches!(self, Self::High)
    }
}

impl fmt::Display for Confidence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confidence_order_matches_documentation() {
        assert!(Confidence::Low < Confidence::Medium);
        assert!(Confidence::Medium < Confidence::High);
    }

    #[test]
    fn detects_high_confidence() {
        assert!(!Confidence::Low.is_high());
        assert!(!Confidence::Medium.is_high());
        assert!(Confidence::High.is_high());
    }
}
