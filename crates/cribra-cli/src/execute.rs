//! Reusable command execution for the canonical Cribra CLI.
//!
//! Execution delegates all detection semantics to the `cribra` core. This
//! module owns only CLI-level source acquisition and presentation selection.

use std::{
    fmt,
    fs::File,
    io::{self, Read},
    path::Path,
};

use crate::{
    command::{Command, ConfidenceFilter, ScanInput, SeverityFilter},
    input, output,
};

/// Result of executing one reusable CLI command.
///
/// Output contains scan metadata, help/version text, or exact Core-transformed
/// source. Transformed source may retain candidates and undetected sensitive
/// data; the CLI never adds matched source values independently.
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

    /// Core could not safely transform the original source and report.
    Transform(cribra::transform::TransformError),

    /// The key file could not be read or was not a regular file.
    KeyRead(std::io::Error),

    /// The key file did not contain exactly 32 raw bytes.
    InvalidKeyLength,

    /// The requested operation is unavailable, with a presentation-safe reason.
    UnsupportedOperation(&'static str),
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
            Self::Transform(error) => error.fmt(formatter),
            Self::KeyRead(error) => write!(formatter, "could not read key file: {error}"),
            Self::InvalidKeyLength => {
                formatter.write_str("key file must contain exactly 32 raw bytes")
            }
            Self::UnsupportedOperation(reason) => formatter.write_str(reason),
        }
    }
}

impl From<cribra::transform::TransformError> for ExecuteError {
    fn from(error: cribra::transform::TransformError) -> Self {
        Self::Transform(error)
    }
}

impl std::error::Error for ExecuteError {}

pub(crate) fn read_key_file(path: &Path) -> Result<[u8; 32], ExecuteError> {
    // Follow symlinks, but reject special files before opening to avoid reading
    // from devices or blocking on a named pipe. Check the opened handle too.
    let metadata = std::fs::metadata(path).map_err(ExecuteError::KeyRead)?;
    if !metadata.is_file() {
        return Err(non_regular_key_file());
    }
    let mut file = File::open(path).map_err(ExecuteError::KeyRead)?;
    if !file.metadata().map_err(ExecuteError::KeyRead)?.is_file() {
        return Err(non_regular_key_file());
    }

    let mut key = [0; 32];
    file.read_exact(&mut key).map_err(|error| {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            ExecuteError::InvalidKeyLength
        } else {
            ExecuteError::KeyRead(error)
        }
    })?;
    // Probe one extra byte without reading or retaining an unbounded file.
    match file.read_exact(&mut [0; 1]) {
        Ok(()) => Err(ExecuteError::InvalidKeyLength),
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => Ok(key),
        Err(error) => Err(ExecuteError::KeyRead(error)),
    }
}

fn non_regular_key_file() -> ExecuteError {
    ExecuteError::KeyRead(io::Error::new(
        io::ErrorKind::InvalidInput,
        "key file must be a regular file",
    ))
}

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

        Command::Redact(input) | Command::Template(input) => {
            let input = match input {
                ScanInput::File(path) => input::Input::File(path.clone()),
                ScanInput::Stdin => input::Input::Stdin,
            };
            let source = input::read(&input)?;
            let scanner = cribra::Scanner::default();
            let results = scanner.scan([(source.name(), source.text())]);
            let report = results
                .single_report()
                .expect("single CLI input must produce exactly one report");

            let transformed = match command {
                Command::Redact(_) => cribra::transform::redact(source.text(), report),
                Command::Template(_) => cribra::transform::template(source.text(), report),
                _ => unreachable!("only transformation commands enter this branch"),
            }?;
            Ok(CommandOutput::new(transformed))
        }

        Command::Pseudonymize(options) | Command::Synthesize(options) => {
            // Validate the key before acquiring the source, especially stdin.
            let key = read_key_file(&options.key_file)?;
            let input = match &options.input {
                ScanInput::File(path) => input::Input::File(path.clone()),
                ScanInput::Stdin => input::Input::Stdin,
            };
            let source = input::read(&input)?;
            let scanner = cribra::Scanner::default();
            let results = scanner.scan([(source.name(), source.text())]);
            let report = results
                .single_report()
                .expect("single CLI input must produce exactly one report");
            let transformed = match command {
                Command::Pseudonymize(_) => cribra::transform::pseudonymize(
                    source.text(),
                    report,
                    &cribra::transform::PseudonymizationOptions::new(key),
                ),
                Command::Synthesize(_) => cribra::transform::synthesize(
                    source.text(),
                    report,
                    &cribra::transform::SynthesisOptions::new(key),
                ),
                _ => unreachable!("only keyed transformation commands enter this branch"),
            }?;
            Ok(CommandOutput::new(transformed))
        }

        Command::Bundle(_) => Err(ExecuteError::UnsupportedOperation(
            "bundle execution is not implemented",
        )),

        Command::Help => Ok(CommandOutput::new(crate::help_text().to_owned())),

        Command::Version => Ok(CommandOutput::new(format!("cribra {}\n", crate::VERSION))),
    }
}
