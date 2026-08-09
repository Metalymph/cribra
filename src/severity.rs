//! Severity assigned to findings produced by detection rules.

use core::fmt;

/// Indicates the impact of a detected finding.
///
/// Severity expresses how important a finding is from a security or operational
/// perspective. It is assigned by the matching rule and does not depend on how
/// confident the scanner is that the finding is correct.
///
/// The ordering of the variants follows their natural importance:
///
/// ```text
/// Info < Low < Medium < High < Critical
/// ```
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Severity {
    /// Informational finding with no immediate action required.
    Info,

    /// Low-impact finding.
    Low,

    /// Medium-impact finding.
    Medium,

    /// High-impact finding.
    High,

    /// Critical finding requiring immediate attention.
    Critical,
}

impl Severity {
    /// Returns `true` when this severity is considered high priority.
    #[must_use]
    pub const fn is_high_priority(self) -> bool {
        matches!(self, Self::High | Self::Critical)
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Info => "info",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_order_matches_documentation() {
        assert!(Severity::Info < Severity::Low);
        assert!(Severity::Low < Severity::Medium);
        assert!(Severity::Medium < Severity::High);
        assert!(Severity::High < Severity::Critical);
    }

    #[test]
    fn detects_high_priority_levels() {
        assert!(!Severity::Info.is_high_priority());
        assert!(!Severity::Medium.is_high_priority());
        assert!(Severity::High.is_high_priority());
        assert!(Severity::Critical.is_high_priority());
    }
}
