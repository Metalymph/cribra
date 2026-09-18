//! Public command model for the reusable Cribra CLI.

use std::path::PathBuf;

use cribra::{Confidence, Severity};

/// A command supported by the canonical Cribra command surface.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Command {
    /// Scan one explicit UTF-8 source.
    Scan(ScanCommand),

    /// Render command-line help.
    Help,

    /// Render the CLI version.
    Version,
}

/// Parsed `scan` command.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ScanCommand {
    /// Source selected by the caller.
    pub input: ScanInput,

    /// Requested output representation.
    pub format: OutputFormat,

    /// Minimum finding severity included in output.
    pub minimum_severity: Option<Severity>,

    /// Minimum finding confidence included in output.
    pub minimum_confidence: Option<Confidence>,
}

/// Explicit scan input.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ScanInput {
    /// Read one explicit filesystem path.
    File(PathBuf),

    /// Read from standard input.
    Stdin,
}

/// Stable CLI output representation.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub enum OutputFormat {
    /// Human-readable metadata output.
    #[default]
    Human,

    /// Machine-readable JSON metadata output.
    Json,
}

impl OutputFormat {
    /// Parses one stable CLI output-format name.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "human" => Some(Self::Human),
            "json" => Some(Self::Json),
            _ => None,
        }
    }
}

pub(crate) fn parse_severity(value: &str) -> Option<Severity> {
    match value {
        "info" => Some(Severity::Info),
        "low" => Some(Severity::Low),
        "medium" => Some(Severity::Medium),
        "high" => Some(Severity::High),
        "critical" => Some(Severity::Critical),
        _ => None,
    }
}

pub(crate) fn parse_confidence(value: &str) -> Option<Confidence> {
    match value {
        "low" => Some(Confidence::Low),
        "medium" => Some(Confidence::Medium),
        "high" => Some(Confidence::High),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_names_are_explicit() {
        assert_eq!(parse_severity("info"), Some(Severity::Info));
        assert_eq!(parse_severity("low"), Some(Severity::Low));
        assert_eq!(parse_severity("medium"), Some(Severity::Medium));
        assert_eq!(parse_severity("high"), Some(Severity::High));
        assert_eq!(parse_severity("critical"), Some(Severity::Critical));
        assert_eq!(parse_severity("unknown"), None);
    }

    #[test]
    fn confidence_names_are_explicit() {
        assert_eq!(parse_confidence("low"), Some(Confidence::Low));
        assert_eq!(parse_confidence("medium"), Some(Confidence::Medium));
        assert_eq!(parse_confidence("high"), Some(Confidence::High));
        assert_eq!(parse_confidence("unknown"), None);
    }
}
