//! Reusable command-line interface for Cribra.
//!
//! This crate owns canonical Cribra command parsing, execution, and CLI
//! presentation semantics.
//!
//! Detection, validation, findings, remediation, and transformations remain
//! authoritative in the `cribra` core crate.

mod command;
mod execute;
mod input;
mod output;

use std::{
    ffi::{OsStr, OsString},
    process::ExitCode,
};

pub use command::{Command, OutputFormat, ScanCommand, ScanInput};
pub use execute::{CommandOutput, ExecuteError, execute};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Command-line parsing failure.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ParseError {
    message: String,
}

impl ParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Returns the stable human-readable parse diagnostic.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ParseError {}

/// Parses canonical Cribra CLI arguments into a reusable command.
///
/// The first argument is treated as the program name and ignored.
pub fn parse<I, S>(args: I) -> Result<Command, ParseError>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let mut args = args.into_iter().map(Into::into);
    let _program = args.next();

    match args.next() {
        None => Ok(Command::Help),

        Some(argument) if argument == OsStr::new("scan") => {
            let Some(input) = args.next() else {
                return Err(ParseError::new(
                    "scan requires an input path or '-' for stdin",
                ));
            };

            let input = if input == OsStr::new("-") {
                ScanInput::Stdin
            } else {
                ScanInput::File(input.into())
            };

            let mut format = OutputFormat::Human;

            let mut minimum_severity = None;
            let mut minimum_confidence = None;

            while let Some(argument) = args.next() {
                if argument == OsStr::new("--format") {
                    let Some(value) = args.next() else {
                        return Err(ParseError::new("--format requires 'human' or 'json'"));
                    };

                    let Some(parsed) = OutputFormat::parse(&value.to_string_lossy()) else {
                        return Err(ParseError::new(format!(
                            "unsupported output format: {}",
                            value.to_string_lossy()
                        )));
                    };

                    format = parsed;
                    continue;
                }

                if argument == OsStr::new("--min-severity") {
                    let Some(value) = args.next() else {
                        return Err(ParseError::new(
                            "--min-severity requires info, low, medium, high, or critical",
                        ));
                    };

                    let Some(parsed) = command::parse_severity(&value.to_string_lossy()) else {
                        return Err(ParseError::new(format!(
                            "unsupported minimum severity: {}",
                            value.to_string_lossy()
                        )));
                    };

                    minimum_severity = Some(parsed);
                    continue;
                }

                if argument == OsStr::new("--min-confidence") {
                    let Some(value) = args.next() else {
                        return Err(ParseError::new(
                            "--min-confidence requires low, medium, or high",
                        ));
                    };

                    let Some(parsed) = command::parse_confidence(&value.to_string_lossy()) else {
                        return Err(ParseError::new(format!(
                            "unsupported minimum confidence: {}",
                            value.to_string_lossy()
                        )));
                    };

                    minimum_confidence = Some(parsed);
                    continue;
                }

                return Err(ParseError::new(format!(
                    "unexpected scan argument: {}",
                    argument.to_string_lossy()
                )));
            }

            Ok(Command::Scan(ScanCommand {
                input,
                format,
                minimum_severity,
                minimum_confidence,
            }))
        }

        Some(argument) if argument == OsStr::new("--help") || argument == OsStr::new("-h") => {
            Ok(Command::Help)
        }

        Some(argument) if argument == OsStr::new("--version") || argument == OsStr::new("-V") => {
            Ok(Command::Version)
        }

        Some(argument) => Err(ParseError::new(format!(
            "unknown argument: {}",
            argument.to_string_lossy()
        ))),
    }
}

/// Executes the Cribra process-style command-line adapter.
///
/// Library consumers should prefer [`parse`] and [`execute`] directly when
/// they do not need process stdout/stderr handling.
pub fn run<I, S>(args: I) -> ExitCode
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let command = match parse(args) {
        Ok(command) => command,

        Err(error) => {
            eprintln!("cribra: {error}");
            eprintln!("Try 'cribra --help' for usage.");
            return ExitCode::from(2);
        }
    };

    match execute(&command) {
        Ok(output) => {
            print!("{}", output.stdout());
            ExitCode::SUCCESS
        }

        Err(error) => {
            eprintln!("cribra: {error}");
            ExitCode::FAILURE
        }
    }
}

pub(crate) fn help_text() -> &'static str {
    "\
    Usage:
      cribra scan <FILE> [--format human|json] [--min-severity LEVEL] [--min-confidence LEVEL]
      cribra scan - [--format human|json] [--min-severity LEVEL] [--min-confidence LEVEL]
      cribra [OPTIONS]

    Commands:
      scan <FILE>              Scan one explicit UTF-8 file
      scan -                   Scan UTF-8 from standard input

    Scan options:
      --format FORMAT          Output format: human or json
      --min-severity LEVEL     Minimum severity: info, low, medium, high, critical
      --min-confidence LEVEL   Minimum confidence: low, medium, high

    Options:
      -h, --help               Print help
      -V, --version            Print version
"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_arguments_parse_as_help() {
        assert_eq!(parse(["cribra"]).unwrap(), Command::Help);
    }

    #[test]
    fn help_parses() {
        assert_eq!(parse(["cribra", "--help"]).unwrap(), Command::Help);
        assert_eq!(parse(["cribra", "-h"]).unwrap(), Command::Help);
    }

    #[test]
    fn version_parses() {
        assert_eq!(parse(["cribra", "--version"]).unwrap(), Command::Version);
        assert_eq!(parse(["cribra", "-V"]).unwrap(), Command::Version);
    }

    #[test]
    fn scan_file_parses() {
        assert_eq!(
            parse(["cribra", "scan", "config.env"]).unwrap(),
            Command::Scan(ScanCommand {
                input: ScanInput::File("config.env".into()),
                format: OutputFormat::Human,
                minimum_severity: None,
                minimum_confidence: None,
            })
        );
    }

    #[test]
    fn scan_stdin_parses() {
        assert_eq!(
            parse(["cribra", "scan", "-", "--format", "json"]).unwrap(),
            Command::Scan(ScanCommand {
                input: ScanInput::Stdin,
                format: OutputFormat::Json,
                minimum_severity: None,
                minimum_confidence: None,
            })
        );
    }

    #[test]
    fn unknown_argument_is_parse_error() {
        assert!(parse(["cribra", "--unknown"]).is_err());
    }

    #[test]
    fn scan_requires_input() {
        assert!(parse(["cribra", "scan"]).is_err());
    }

    #[test]
    fn scan_rejects_multiple_inputs() {
        assert!(parse(["cribra", "scan", "one.env", "two.env"]).is_err());
    }

    #[test]
    fn reusable_execute_returns_output_without_process_capture() {
        use std::{
            fs,
            time::{SystemTime, UNIX_EPOCH},
        };

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();

        let path = std::env::temp_dir().join(format!(
            "cribra-cli-execute-{}-{unique}.env",
            std::process::id()
        ));

        fs::write(
            &path,
            b"GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789\n",
        )
        .unwrap();

        let command = Command::Scan(ScanCommand {
            input: ScanInput::File(path.clone()),
            format: OutputFormat::Json,
            minimum_severity: None,
            minimum_confidence: None,
        });

        let result = execute(&command).unwrap();

        fs::remove_file(path).unwrap();

        assert!(result.stdout().contains("\"status\":\"findings\""));
        assert!(
            !result
                .stdout()
                .contains("ghp_AbCdEf0123456789_AbCdEf0123456789")
        );
    }

    #[test]
    fn process_adapter_preserves_usage_exit_code() {
        assert_eq!(run(["cribra", "--unknown"]), ExitCode::from(2));
    }

    #[test]
    fn scan_filters_parse() {
        assert_eq!(
            parse([
                "cribra",
                "scan",
                "config.env",
                "--min-severity",
                "high",
                "--min-confidence",
                "medium",
            ])
            .unwrap(),
            Command::Scan(ScanCommand {
                input: ScanInput::File("config.env".into()),
                format: OutputFormat::Human,
                minimum_severity: Some(cribra::Severity::High),
                minimum_confidence: Some(cribra::Confidence::Medium),
            })
        );
    }

    #[test]
    fn scan_rejects_invalid_minimum_severity() {
        assert!(parse(["cribra", "scan", "config.env", "--min-severity", "urgent",]).is_err());
    }

    #[test]
    fn scan_rejects_invalid_minimum_confidence() {
        assert!(
            parse([
                "cribra",
                "scan",
                "config.env",
                "--min-confidence",
                "certain",
            ])
            .is_err()
        );
    }

    #[test]
    fn minimum_severity_includes_equal_severity() {
        use std::{
            fs,
            time::{SystemTime, UNIX_EPOCH},
        };

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();

        let path = std::env::temp_dir().join(format!(
            "cribra-cli-filter-{}-{unique}.env",
            std::process::id()
        ));

        fs::write(
            &path,
            b"GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789\n",
        )
        .unwrap();

        let command = Command::Scan(ScanCommand {
            input: ScanInput::File(path.clone()),
            format: OutputFormat::Json,
            minimum_severity: Some(cribra::Severity::Critical),
            minimum_confidence: None,
        });

        let result = execute(&command).unwrap();

        fs::remove_file(path).unwrap();

        assert!(result.stdout().contains("\"findings_count\":1"));
        assert!(result.stdout().contains("\"status\":\"findings\""));
        assert!(result.stdout().contains("\"severity\":\"critical\""));
    }
}
