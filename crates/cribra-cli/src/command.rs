//! Public command model for the reusable Cribra CLI.

use std::path::PathBuf;

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
