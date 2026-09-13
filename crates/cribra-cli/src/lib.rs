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

                return Err(ParseError::new(format!(
                    "unexpected scan argument: {}",
                    argument.to_string_lossy()
                )));
            }

            Ok(Command::Scan(ScanCommand { input, format }))
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
  cribra scan <FILE> [--format human|json]
  cribra scan - [--format human|json]
  cribra [OPTIONS]

Commands:
  scan <FILE>       Scan one explicit UTF-8 file
  scan -            Scan UTF-8 from standard input

Scan options:
  --format FORMAT   Output format: human or json

Options:
  -h, --help        Print help
  -V, --version     Print version
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
}
