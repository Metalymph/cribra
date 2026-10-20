#![forbid(unsafe_code)]

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

pub use command::{
    Command, ConfidenceFilter, KeyedTransformCommand, OutputFormat, ScanCommand, ScanFilters,
    ScanInput, SeverityFilter,
};
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

            let mut format = None;
            let mut filters = ScanFilters::default();
            let mut sort = None;
            let mut explain = false;

            while let Some(argument) = args.next() {
                let option = argument.to_string_lossy();
                let already_set = match option.as_ref() {
                    "--format" => format.is_some(),
                    "--severity" | "--min-severity" => filters.severity.is_some(),
                    "--confidence" | "--min-confidence" => filters.confidence.is_some(),
                    "--rule-id" => filters.rule_id.is_some(),
                    "--sort" => sort.is_some(),
                    "--explain" => explain,
                    _ => {
                        return Err(ParseError::new(format!(
                            "unexpected scan argument: {option}"
                        )));
                    }
                };
                if already_set {
                    return Err(ParseError::new(format!(
                        "duplicate or conflicting scan option: {option}"
                    )));
                }

                if option == "--explain" {
                    explain = true;
                    continue;
                }

                let value = args
                    .next()
                    .ok_or_else(|| ParseError::new(format!("{option} requires a value")))?;
                let value = value.to_str().ok_or_else(|| {
                    ParseError::new(format!("{option} requires a valid UTF-8 value"))
                })?;
                if value.is_empty() || value.starts_with("--") {
                    return Err(ParseError::new(format!("{option} requires a value")));
                }

                let invalid_value =
                    || ParseError::new(format!("unsupported value for {option}: {value}"));
                match option.as_ref() {
                    "--format" => {
                        format = Some(OutputFormat::parse(value).ok_or_else(invalid_value)?);
                    }
                    "--severity" | "--min-severity" => {
                        let severity = parse_severity(value).ok_or_else(invalid_value)?;
                        filters.severity = Some(if option == "--severity" {
                            SeverityFilter::Exact(severity)
                        } else {
                            SeverityFilter::Minimum(severity)
                        });
                    }
                    "--confidence" | "--min-confidence" => {
                        let confidence = parse_confidence(value).ok_or_else(invalid_value)?;
                        filters.confidence = Some(if option == "--confidence" {
                            ConfidenceFilter::Exact(confidence)
                        } else {
                            ConfidenceFilter::Minimum(confidence)
                        });
                    }
                    "--rule-id" => filters.rule_id = Some(value.to_owned()),
                    "--sort" => sort = Some(parse_sort(value).ok_or_else(invalid_value)?),
                    _ => unreachable!("scan option was checked before reading its value"),
                }
            }

            Ok(Command::Scan(ScanCommand {
                input,
                format: format.unwrap_or_default(),
                filters,
                sort,
                explain,
            }))
        }

        Some(argument)
            if argument == OsStr::new("redact") || argument == OsStr::new("template") =>
        {
            let name = argument.to_string_lossy();
            let input = args.next().ok_or_else(|| {
                ParseError::new(format!("{name} requires an input path or '-' for stdin"))
            })?;
            if (input != OsStr::new("-") && input.as_encoded_bytes().starts_with(b"-"))
                || args.next().is_some()
            {
                return Err(ParseError::new(format!(
                    "{name} requires exactly one input and accepts no options"
                )));
            }
            let input = if input == OsStr::new("-") {
                ScanInput::Stdin
            } else {
                ScanInput::File(input.into())
            };
            Ok(if argument == OsStr::new("redact") {
                Command::Redact(input)
            } else {
                Command::Template(input)
            })
        }

        Some(argument)
            if argument == OsStr::new("pseudonymize") || argument == OsStr::new("synthesize") =>
        {
            let command = parse_keyed_transform(args)?;
            Ok(if argument == OsStr::new("pseudonymize") {
                Command::Pseudonymize(command)
            } else {
                Command::Synthesize(command)
            })
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

fn parse_keyed_transform(
    mut args: impl Iterator<Item = OsString>,
) -> Result<KeyedTransformCommand, ParseError> {
    let mut input = None;
    let mut key_file = None;
    while let Some(argument) = args.next() {
        if argument == OsStr::new("--key-file") {
            if key_file.is_some() {
                return Err(ParseError::new("duplicate --key-file option"));
            }
            let path = args
                .next()
                .ok_or_else(|| ParseError::new("--key-file requires a path"))?;
            if path.is_empty() || path.as_encoded_bytes().starts_with(b"-") {
                return Err(ParseError::new("--key-file requires a filesystem path"));
            }
            key_file = Some(path.into());
        } else {
            if argument != OsStr::new("-") && argument.as_encoded_bytes().starts_with(b"-") {
                return Err(ParseError::new("unsupported keyed transformation option"));
            }
            if input.is_some() {
                return Err(ParseError::new(
                    "keyed transformations require exactly one source",
                ));
            }
            input = Some(if argument == OsStr::new("-") {
                ScanInput::Stdin
            } else {
                ScanInput::File(argument.into())
            });
        }
    }
    Ok(KeyedTransformCommand {
        input: input
            .ok_or_else(|| ParseError::new("a source path or '-' for stdin is required"))?,
        key_file: key_file.ok_or_else(|| ParseError::new("--key-file is required"))?,
    })
}

fn parse_severity(value: &str) -> Option<cribra::Severity> {
    match value {
        "info" => Some(cribra::Severity::Info),
        "low" => Some(cribra::Severity::Low),
        "medium" => Some(cribra::Severity::Medium),
        "high" => Some(cribra::Severity::High),
        "critical" => Some(cribra::Severity::Critical),
        _ => None,
    }
}

fn parse_confidence(value: &str) -> Option<cribra::Confidence> {
    match value {
        "low" => Some(cribra::Confidence::Low),
        "medium" => Some(cribra::Confidence::Medium),
        "high" => Some(cribra::Confidence::High),
        _ => None,
    }
}

fn parse_sort(value: &str) -> Option<cribra::ScanSort> {
    match value {
        "rule-id" => Some(cribra::ScanSort::RuleId),
        "rule-id-desc" => Some(cribra::ScanSort::RuleIdDescending),
        "severity" => Some(cribra::ScanSort::Severity),
        "severity-desc" => Some(cribra::ScanSort::SeverityDescending),
        "confidence" => Some(cribra::ScanSort::Confidence),
        "confidence-desc" => Some(cribra::ScanSort::ConfidenceDescending),
        "location" => Some(cribra::ScanSort::Location),
        "location-desc" => Some(cribra::ScanSort::LocationDescending),
        _ => None,
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
  cribra scan <FILE> [SCAN OPTIONS]
  cribra scan - [SCAN OPTIONS]
  cribra redact <FILE|->
  cribra template <FILE|->
  cribra pseudonymize <FILE|-> --key-file <PATH>
  cribra synthesize <FILE|-> --key-file <PATH>
  cribra [OPTIONS]

Commands:
  scan <FILE>       Scan one explicit UTF-8 file
  scan -            Scan UTF-8 from standard input
  redact <FILE|->   Replace all findings with [REDACTED]
  template <FILE|-> Replace all findings with <CRIBRA:rule-id>
  pseudonymize      Transform findings to stable keyed pseudonyms
  synthesize        Transform findings to keyed synthetic values

Transformations emit only transformed text, without an added newline.
Candidates and undetected sensitive data may remain.
Redact and template accept no options. Keyed commands require --key-file PATH
before or after the source: exactly 32 raw binary bytes in a regular file.
Keys are never read from stdin, inline arguments, or environment variables.

Scan options:
  --format FORMAT         Output format: human or json
  --severity LEVEL        Exact severity: info|low|medium|high|critical
  --min-severity LEVEL    Minimum severity: info|low|medium|high|critical
  --confidence LEVEL      Exact confidence: low|medium|high
  --min-confidence LEVEL  Minimum confidence: low|medium|high
  --rule-id ID            Exact rule ID (nonempty)
  --sort ORDER            rule-id|rule-id-desc|severity|severity-desc|
                          confidence|confidence-desc|location|location-desc
  --explain               Include finding detection modes
  Exact and minimum constraints in the same dimension are mutually exclusive.

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
    fn transformation_commands_parse_file_and_stdin() {
        for input in ["config.env", "-"] {
            let expected = if input == "-" {
                ScanInput::Stdin
            } else {
                ScanInput::File(input.into())
            };
            assert_eq!(
                parse(["cribra", "redact", input]).unwrap(),
                Command::Redact(expected.clone())
            );
            assert_eq!(
                parse(["cribra", "template", input]).unwrap(),
                Command::Template(expected)
            );
        }
    }

    #[test]
    fn keyed_commands_parse_file_and_stdin_in_either_order() {
        for name in ["pseudonymize", "synthesize"] {
            for source in ["config.env", "-"] {
                let expected = KeyedTransformCommand {
                    input: if source == "-" {
                        ScanInput::Stdin
                    } else {
                        ScanInput::File(source.into())
                    },
                    key_file: "key.bin".into(),
                };
                let expected = if name == "pseudonymize" {
                    Command::Pseudonymize(expected)
                } else {
                    Command::Synthesize(expected)
                };
                for args in [
                    ["cribra", name, source, "--key-file", "key.bin"],
                    ["cribra", name, "--key-file", "key.bin", source],
                ] {
                    assert_eq!(parse(args).unwrap(), expected);
                }
            }
        }
    }

    #[test]
    fn keyed_commands_require_one_source_and_one_key_path() {
        for name in ["pseudonymize", "synthesize"] {
            for args in [
                &[][..],
                &["-"][..],
                &["--key-file", "key.bin"][..],
                &["-", "--key-file"][..],
                &["-", "--key-file", ""][..],
                &["-", "--key-file", "-"][..],
                &["-", "--key-file", "--explain"][..],
                &["-", "--key-file", "key.bin", "--key-file", "other.bin"][..],
                &["one.env", "--key-file", "key.bin", "two.env"][..],
            ] {
                assert!(parse(["cribra", name].into_iter().chain(args.iter().copied())).is_err());
            }
        }
    }

    #[test]
    fn keyed_commands_reject_all_other_options() {
        for name in ["pseudonymize", "synthesize"] {
            for option in [
                "--format",
                "--explain",
                "--severity",
                "--min-severity",
                "--confidence",
                "--min-confidence",
                "--rule-id",
                "--sort",
                "--key",
                "--key-env",
                "--generate-key",
                "--prefix",
                "--marker",
                "--output",
                "--help",
                "--key-file=key.bin",
                "--unknown",
            ] {
                assert!(parse(["cribra", name, "-", "--key-file", "key.bin", option]).is_err());
                assert!(parse(["cribra", name, option, "-", "--key-file", "key.bin"]).is_err());
            }
        }
    }

    #[test]
    fn transformation_commands_require_exactly_one_input() {
        for command in ["redact", "template"] {
            assert!(parse(["cribra", command]).is_err());
            assert!(parse(["cribra", command, "one.env", "two.env"]).is_err());
            assert!(parse(["cribra", command, "-", "two.env"]).is_err());
        }
    }

    #[test]
    fn transformation_commands_reject_options() {
        for command in ["redact", "template"] {
            for option in [
                "--format",
                "--explain",
                "--severity",
                "--min-severity",
                "--confidence",
                "--min-confidence",
                "--rule-id",
                "--sort",
                "--replacement",
                "--namespace",
                "--numbered",
                "--output",
                "--output-file",
                "--unknown",
                "--help",
                "-h",
                "--",
            ] {
                assert!(parse(["cribra", command, option]).is_err());
                assert!(parse(["cribra", command, "-", option]).is_err());
                assert!(parse(["cribra", command, "config.env", option, "value"]).is_err());
            }
        }
    }

    #[test]
    fn scan_file_parses() {
        assert_eq!(
            parse(["cribra", "scan", "config.env"]).unwrap(),
            Command::Scan(ScanCommand {
                input: ScanInput::File("config.env".into()),
                format: OutputFormat::Human,
                filters: ScanFilters::default(),
                sort: None,
                explain: false,
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
                filters: ScanFilters::default(),
                sort: None,
                explain: false,
            })
        );
    }

    #[test]
    fn scan_explain_parses_for_file_and_stdin() {
        for input in ["config.env", "-"] {
            let Command::Scan(command) = parse(["cribra", "scan", input, "--explain"]).unwrap()
            else {
                panic!("expected scan command");
            };
            assert_eq!(
                command,
                ScanCommand {
                    input: if input == "-" {
                        ScanInput::Stdin
                    } else {
                        ScanInput::File(input.into())
                    },
                    format: OutputFormat::Human,
                    filters: ScanFilters::default(),
                    sort: None,
                    explain: true,
                }
            );
        }
    }

    #[test]
    fn scan_explain_composes_with_options_in_any_position() {
        let args = [
            "cribra",
            "scan",
            "-",
            "--severity",
            "high",
            "--min-confidence",
            "medium",
            "--rule-id",
            "generic.api-key",
            "--sort",
            "rule-id",
            "--format",
            "json",
        ];
        let Command::Scan(mut expected) = parse(args).unwrap() else {
            panic!("expected scan command");
        };
        expected.explain = true;
        for position in (3..=args.len()).step_by(2) {
            let mut with_explain = args.to_vec();
            with_explain.insert(position, "--explain");
            assert_eq!(
                parse(with_explain).unwrap(),
                Command::Scan(expected.clone())
            );
        }
    }

    #[test]
    fn scan_rejects_duplicate_explain_and_flag_values() {
        assert!(parse(["cribra", "scan", "-", "--explain", "--explain"]).is_err());
        assert!(
            parse([
                "cribra",
                "scan",
                "-",
                "--explain",
                "--sort",
                "severity",
                "--explain"
            ])
            .is_err()
        );
        assert!(parse(["cribra", "scan", "-", "--explain", "true"]).is_err());
        assert!(parse(["cribra", "scan", "-", "--explain=true"]).is_err());
    }

    #[test]
    fn scan_query_options_are_order_independent() {
        let options = [
            ["--severity", "high"],
            ["--min-confidence", "medium"],
            ["--rule-id", "github.classic-pat"],
            ["--sort", "location-desc"],
            ["--format", "json"],
        ];
        for input in ["config.env", "-"] {
            for offset in 0..options.len() {
                let args = ["cribra", "scan", input].into_iter().chain(
                    options
                        .iter()
                        .cycle()
                        .skip(offset)
                        .take(options.len())
                        .flatten()
                        .copied(),
                );
                assert_eq!(
                    parse(args).unwrap(),
                    Command::Scan(ScanCommand {
                        input: if input == "-" {
                            ScanInput::Stdin
                        } else {
                            ScanInput::File(input.into())
                        },
                        format: OutputFormat::Json,
                        filters: ScanFilters {
                            severity: Some(SeverityFilter::Exact(cribra::Severity::High)),
                            confidence: Some(ConfidenceFilter::Minimum(cribra::Confidence::Medium)),
                            rule_id: Some("github.classic-pat".into()),
                        },
                        sort: Some(cribra::ScanSort::LocationDescending),
                        explain: false,
                    })
                );
            }
        }
    }

    #[test]
    fn scan_accepts_all_severity_constraints() {
        for (value, severity) in [
            ("info", cribra::Severity::Info),
            ("low", cribra::Severity::Low),
            ("medium", cribra::Severity::Medium),
            ("high", cribra::Severity::High),
            ("critical", cribra::Severity::Critical),
        ] {
            for (option, expected) in [
                ("--severity", SeverityFilter::Exact(severity)),
                ("--min-severity", SeverityFilter::Minimum(severity)),
            ] {
                let Command::Scan(command) = parse(["cribra", "scan", "-", option, value]).unwrap()
                else {
                    panic!("expected scan command");
                };
                assert_eq!(command.filters.severity, Some(expected));
            }
        }
    }

    #[test]
    fn scan_accepts_all_confidence_constraints() {
        for (value, confidence) in [
            ("low", cribra::Confidence::Low),
            ("medium", cribra::Confidence::Medium),
            ("high", cribra::Confidence::High),
        ] {
            for (option, expected) in [
                ("--confidence", ConfidenceFilter::Exact(confidence)),
                ("--min-confidence", ConfidenceFilter::Minimum(confidence)),
            ] {
                let Command::Scan(command) = parse(["cribra", "scan", "-", option, value]).unwrap()
                else {
                    panic!("expected scan command");
                };
                assert_eq!(command.filters.confidence, Some(expected));
            }
        }
    }

    #[test]
    fn scan_accepts_all_supported_sorts() {
        for (value, expected) in [
            ("rule-id", cribra::ScanSort::RuleId),
            ("rule-id-desc", cribra::ScanSort::RuleIdDescending),
            ("severity", cribra::ScanSort::Severity),
            ("severity-desc", cribra::ScanSort::SeverityDescending),
            ("confidence", cribra::ScanSort::Confidence),
            ("confidence-desc", cribra::ScanSort::ConfidenceDescending),
            ("location", cribra::ScanSort::Location),
            ("location-desc", cribra::ScanSort::LocationDescending),
        ] {
            let Command::Scan(command) = parse(["cribra", "scan", "-", "--sort", value]).unwrap()
            else {
                panic!("expected scan command");
            };
            assert_eq!(command.sort, Some(expected));
            assert_eq!(command.filters, ScanFilters::default());
        }
    }

    #[test]
    fn scan_rejects_duplicate_options() {
        for (option, first, second) in [
            ("--format", "human", "json"),
            ("--severity", "low", "high"),
            ("--min-severity", "low", "high"),
            ("--confidence", "low", "high"),
            ("--min-confidence", "low", "high"),
            ("--rule-id", "one", "two"),
            ("--sort", "severity", "confidence"),
        ] {
            for value in [first, second] {
                let error =
                    parse(["cribra", "scan", "-", option, first, option, value]).unwrap_err();
                assert!(error.message().contains("duplicate or conflicting"));
            }
        }
    }

    #[test]
    fn scan_rejects_exact_minimum_conflicts_in_either_order() {
        for (exact, minimum) in [
            ("--severity", "--min-severity"),
            ("--confidence", "--min-confidence"),
        ] {
            for (first, second) in [(exact, minimum), (minimum, exact)] {
                let error =
                    parse(["cribra", "scan", "-", first, "low", second, "high"]).unwrap_err();
                assert!(error.message().contains("duplicate or conflicting"));
            }
        }
    }

    #[test]
    fn scan_rejects_missing_option_values() {
        for option in [
            "--format",
            "--severity",
            "--min-severity",
            "--confidence",
            "--min-confidence",
            "--rule-id",
            "--sort",
        ] {
            assert!(parse(["cribra", "scan", "-", option]).is_err());
            assert!(parse(["cribra", "scan", "-", option, "--format", "json"]).is_err());
            assert!(parse(["cribra", "scan", "-", option, ""]).is_err());
        }
    }

    #[test]
    fn scan_rejects_invalid_option_values_and_unknown_options() {
        for (option, value) in [
            ("--format", "yaml"),
            ("--severity", "urgent"),
            ("--min-severity", "High"),
            ("--confidence", "critical"),
            ("--min-confidence", "info"),
            ("--sort", "source"),
            ("--sort", "source-desc"),
            ("--sort", "unknown"),
            ("--unknown", "high"),
        ] {
            assert!(parse(["cribra", "scan", "-", option, value]).is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    fn scan_rejects_non_utf8_option_values() {
        use std::os::unix::ffi::OsStringExt;

        assert!(
            parse([
                OsString::from("cribra"),
                OsString::from("scan"),
                OsString::from("-"),
                OsString::from("--rule-id"),
                OsString::from_vec(vec![0xff]),
            ])
            .is_err()
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
            filters: ScanFilters::default(),
            sort: None,
            explain: false,
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
