//! Public command model for the reusable Cribra CLI.

use std::path::PathBuf;

/// A command supported by the canonical Cribra command surface.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Command {
    /// Scan one explicit UTF-8 source.
    Scan(ScanCommand),

    /// Redact all findings in one explicit UTF-8 source using Core defaults.
    Redact(ScanInput),

    /// Template all findings in one explicit UTF-8 source using Core defaults.
    Template(ScanInput),

    /// Pseudonymize all findings using an explicit binary key file.
    Pseudonymize(KeyedTransformCommand),

    /// Synthesize all findings using an explicit binary key file.
    Synthesize(KeyedTransformCommand),

    /// Render command-line help.
    Help,

    /// Render the CLI version.
    Version,
}

/// A keyed transformation of one explicit UTF-8 source.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct KeyedTransformCommand {
    /// Source selected by the caller.
    pub input: ScanInput,

    /// Regular file containing exactly 32 raw binary key bytes.
    pub key_file: PathBuf,
}

/// Parsed `scan` command.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ScanCommand {
    /// Source selected by the caller.
    pub input: ScanInput,

    /// Requested output representation.
    pub format: OutputFormat,

    /// Requested finding filters, recorded independently of execution.
    pub filters: ScanFilters,

    /// Requested finding order, or the default source order when absent.
    pub sort: Option<cribra::ScanSort>,

    /// Include scanner-authoritative detection modes for rendered findings.
    pub explain: bool,
}

/// Optional finding constraints parsed from scan options.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct ScanFilters {
    /// Exact or minimum finding severity.
    pub severity: Option<SeverityFilter>,

    /// Exact or minimum finding confidence.
    pub confidence: Option<ConfidenceFilter>,

    /// Exact rule identifier.
    pub rule_id: Option<String>,
}

/// A finding severity constraint.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SeverityFilter {
    /// Match only this severity.
    Exact(cribra::Severity),

    /// Match this severity and higher severities.
    Minimum(cribra::Severity),
}

/// A finding confidence constraint.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ConfidenceFilter {
    /// Match only this confidence.
    Exact(cribra::Confidence),

    /// Match this confidence and higher confidence levels.
    Minimum(cribra::Confidence),
}

/// Explicit UTF-8 command input.
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
