//! Reusable command execution for the canonical Cribra CLI.
//!
//! Execution delegates all detection semantics to the `cribra` core. This
//! module owns only CLI-level source acquisition and presentation selection.

use std::fmt;

use crate::{
    command::{Command, ConfidenceFilter, ScanInput, SeverityFilter},
    input, output,
};

/// Result of executing one reusable CLI command.
///
/// Output contains presentation-safe CLI text only. Matched source values are
/// never added independently by the CLI layer.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CommandOutput {
    stdout: String,
}

impl CommandOutput {
    /// Creates command output from presentation-safe rendered text.
    fn new(stdout: String) -> Self {
        Self { stdout }
    }

    /// Returns the rendered standard-output payload.
    pub fn stdout(&self) -> &str {
        &self.stdout
    }

    /// Consumes the result and returns its rendered output.
    pub fn into_stdout(self) -> String {
        self.stdout
    }
}

/// Failure while executing a parsed command.
#[derive(Debug)]
pub enum ExecuteError {
    /// The selected input could not be read.
    InputRead(std::io::Error),

    /// The selected input was not valid UTF-8.
    InvalidUtf8,
}

impl From<input::InputError> for ExecuteError {
    fn from(error: input::InputError) -> Self {
        match error {
            input::InputError::Io(error) => Self::InputRead(error),
            input::InputError::InvalidUtf8 => Self::InvalidUtf8,
        }
    }
}

impl fmt::Display for ExecuteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputRead(error) => error.fmt(formatter),
            Self::InvalidUtf8 => formatter.write_str("invalid UTF-8"),
        }
    }
}

impl std::error::Error for ExecuteError {}

/// Executes one already parsed Cribra command.
///
/// This API performs no argument parsing and does not write to stdout or
/// stderr. Consumers can therefore reuse canonical Cribra CLI semantics
/// without spawning the standalone `cribra` executable.
pub fn execute(command: &Command) -> Result<CommandOutput, ExecuteError> {
    match command {
        Command::Scan(command) => {
            let input = match &command.input {
                ScanInput::File(path) => input::Input::File(path.clone()),
                ScanInput::Stdin => input::Input::Stdin,
            };

            let source = input::read(&input)?;
            let scanner = cribra::Scanner::default();
            let results = scanner.scan([(source.name(), source.text())]);

            let report = results
                .single_report()
                .expect("single CLI input must produce exactly one report");

            let filters = &command.filters;
            let selection_active = filters.severity.is_some()
                || filters.confidence.is_some()
                || filters.rule_id.is_some()
                || command.sort.is_some();
            let mut query = results.query();
            if let Some(severity) = filters.severity {
                query = match severity {
                    SeverityFilter::Exact(value) => query.severity(value),
                    SeverityFilter::Minimum(value) => query.minimum_severity(value),
                };
            }
            if let Some(confidence) = filters.confidence {
                query = match confidence {
                    ConfidenceFilter::Exact(value) => query.confidence(value),
                    ConfidenceFilter::Minimum(value) => query.minimum_confidence(value),
                };
            }
            if let Some(rule_id) = filters.rule_id.as_deref() {
                query = query.rule_id(rule_id);
            }

            let stdout = if let Some(sort) = command.sort {
                let sorted = query.sort(sort);
                output::render(
                    command.format,
                    source.name(),
                    report,
                    sorted.iter().map(|(_, finding)| finding),
                    Some(sorted.len()),
                    command.explain.then_some(&scanner),
                )
            } else {
                output::render(
                    command.format,
                    source.name(),
                    report,
                    query.iter().map(|(_, finding)| finding),
                    selection_active.then(|| query.count()),
                    command.explain.then_some(&scanner),
                )
            };

            Ok(CommandOutput::new(stdout))
        }

        Command::Help => Ok(CommandOutput::new(crate::help_text().to_owned())),

        Command::Version => Ok(CommandOutput::new(format!("cribra {}\n", crate::VERSION))),
    }
}
